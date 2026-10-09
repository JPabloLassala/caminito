// The global menu extension's core (ADR 0018): it serves the menu and claims it for its
// window while Caminito offers the global menu. It takes the editor's API as a value, which
// is the seam its tests use (116).

import { readFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import { join } from 'node:path';

import { Message, MessageType, sessionBus } from '@jellybrick/dbus-next';
import type { MessageBus, Variant } from '@jellybrick/dbus-next';

import annotations from '../menu/annotations.json' with { type: 'json' };
import table from '../menu/table.json' with { type: 'json' };
import { DbusMenu } from './dbusmenu.mts';
import type { Disposable, Editor } from './editor.mts';
import { entriesIn, fit, settingsOf } from './menu-table.mts';
import type { Annotations, Table, Workspace } from './menu-table.mts';
import {
  CHANGES_THE_LIST,
  GET_RECENTLY_OPENED,
  MORE,
  RecentItems,
  fallbackOf,
  parseRecent,
} from './recent.mts';
import type { RecentlyOpened } from './recent.mts';

/** The oldest release with every stable API the extension uses, as package.json's `engines.vscode`. */
export const FLOOR = '1.68.0';

/** How long the menu waits after a change for others, so that they're sent as one update. */
export const COALESCING_MS = 50;

/** Where the menu is served, on the connection's unique name. */
export const MENU_PATH = '/org/mate/Caminito/GlobalMenuExtension/MenuBar';

const CAMINITO = 'org.mate.Caminito';
const CAMINITO_PATH = '/org/mate/Caminito';
const OFFERED = 'GlobalMenuOffered';
const DBUS = 'org.freedesktop.DBus';
const PROPERTIES = 'org.freedesktop.DBus.Properties';

/** A menu table and its annotations (ADR 0019). */
export interface Menu {
  readonly table: Table;
  readonly annotations: Annotations;
}

/** VS Code's menu, as menu/table.json and menu/annotations.json have it. */
export const VS_CODE: Menu = { table: table as Table, annotations: annotations as Annotations };

/** Where the extension runs. */
export interface Environment {
  readonly platform: string;
  /** The session bus's address, or `DBUS_SESSION_BUS_ADDRESS`'s when there's none. */
  readonly busAddress?: string;
  /** The menu to serve, VS Code's by default. */
  readonly menu?: Menu;
  /** The home directory, shown as `~` in Open Recent; the user's by default. */
  readonly home?: string;
}

/** The running extension. */
export interface GlobalMenu extends Disposable {
  /** Settles once what was there at activation is known and acted on. */
  readonly ready: Promise<void>;
}

/**
 * Starts the extension. It never throws, and a failure is logged and leaves it idle, so
 * nothing reaches the extension host (ADR 0022).
 */
export function start(editor: Editor, environment: Environment): GlobalMenu {
  // Nothing is served or claimed off Linux, and no log is opened.
  if (environment.platform !== 'linux') {
    return { ready: Promise.resolve(), dispose: () => {} };
  }
  const log = lazyLog(editor);
  const disposables = new Disposables(log);
  // A failed start keeps its log open, with the line that says why.
  const ready = run(editor, environment, log, disposables).catch((err: unknown) => {
    log.line(`can't start: ${message(err)}`);
    disposables.stop();
  });
  return {
    ready,
    dispose: guarded(log, () => {
      disposables.stop();
      log.dispose();
    }),
  };
}

async function run(
  editor: Editor,
  environment: Environment,
  log: Log,
  disposables: Disposables,
): Promise<void> {
  const lateEvents = eventsNewerThanOneSixtySix(editor);
  const missing = lateEvents.filter(([, event]) => event === undefined).map(([name]) => name);
  if (missing.length > 0 || olderThan(editor.version, FLOOR)) {
    const without = missing.length > 0 ? `, so no claim on ${missing.join(' or ')}` : '';
    log.line(`the editor is ${editor.version} and the extension's floor is ${FLOOR}${without}`);
  }

  const bus = sessionBus(environment.busAddress ? { busAddress: environment.busAddress } : {});
  disposables.add({ dispose: () => bus.disconnect() });
  // MessageBus's types leave out the 'error' it emits, which throws if nothing listens. A
  // bus that can't be reached leaves every call unanswered, so its error ends the start.
  const errors = bus as unknown as NodeJS.EventEmitter;
  await new Promise<void>((resolve, reject) => {
    bus.once('connect', resolve);
    errors.once('error', reject);
  });
  errors.removeAllListeners('error');
  errors.on('error', (err: unknown) => log.line(`can't use the session bus: ${message(err)}`));

  const served = environment.menu ?? VS_CODE;
  if (served.table.version !== editor.version) {
    log.line(
      `the menu table is VS Code ${served.table.version}'s and the editor is ${editor.version}`,
    );
  }
  // A list of commands that can't be read keeps every item.
  let commands = await listCommands(editor, log, 'so every item is kept');
  const product = await productOf(editor, log);
  // Open Recent's entries follow the table's own entries, whose ids `fit` gives.
  const recentItems = new RecentItems(
    entriesIn(served.table.menus) + 1,
    environment.home ?? homedir(),
  );
  const recentReader = new RecentReader(editor, log);
  let recent = await recentReader.read();
  const build = () => {
    return fit(served.table.menus, served.annotations, {
      hasCommand: (command) => commands?.has(command) ?? true,
      product,
      setting: settingReader(editor),
      workspace: workspaceOf(editor),
      before: (command) => (command === MORE && recent ? recentItems.items(recent) : []),
    });
  };
  const runCommand = async (command: string, args: readonly unknown[]) => {
    try {
      await editor.commands.executeCommand(command, ...args);
    } catch (err) {
      const fallback = fallbackOf(command, args);
      if (fallback === undefined) {
        log.line(`can't run ${command}: ${message(err)}`);
        return;
      }
      log.line(`can't run ${command}, so ${fallback[0]} is run: ${message(err)}`);
      await runCommand(...fallback);
    }
  };
  const menu = new DbusMenu(build(), async (command, args) => {
    await runCommand(command, args);
    // The list moved or emptied once the command returns, opened or cleared.
    if (CHANGES_THE_LIST.includes(command)) {
      schedule({ recent: true });
    }
  });

  // Updates (ADR 0020): an event marks what to re-read, and the menu is built again once
  // the changes that came with it are in. Nothing else re-reads, so there's no timer.
  let pending: ReturnType<typeof setTimeout> | undefined;
  let relist = false;
  let reread = false;
  let updating = Promise.resolve();
  let stopped = false;
  const apply = async () => {
    if (relist) {
      relist = false;
      // A list that can't be read again keeps the last one.
      commands = (await listCommands(editor, log, 'so the last list is kept')) ?? commands;
    }
    if (reread) {
      reread = false;
      recent = await recentReader.read();
    }
    if (!stopped) {
      menu.update(build());
    }
  };
  /** Settings and folders are always read again; the commands and the recent list when asked. */
  const schedule = (also: { commands?: boolean; recent?: boolean } = {}) => {
    relist ||= also.commands === true;
    reread ||= also.recent === true;
    pending ??= setTimeout(() => {
      pending = undefined;
      // One at a time, so that an older list of commands never lands last.
      updating = updating
        .then(apply)
        .catch((err: unknown) => log.line(`can't update the menu: ${message(err)}`));
    }, COALESCING_MS);
  };
  disposables.add({
    dispose: () => {
      stopped = true;
      clearTimeout(pending);
    },
  });
  const annotated = settingsOf(served.annotations);
  disposables.add(
    editor.workspace.onDidChangeConfiguration(
      guarded(log, (event: { affectsConfiguration(section: string): boolean }) => {
        if (annotated.some((name) => event.affectsConfiguration(name))) {
          schedule();
        }
      }),
    ),
  );
  disposables.add(editor.workspace.onDidChangeWorkspaceFolders(() => schedule()));
  disposables.add(editor.extensions.onDidChange(() => schedule({ commands: true })));
  // The recent list has no event of its own. Focus lost isn't one: a panel menu opening
  // takes the focus, and the menu would be replaced as it opens.
  disposables.add(
    editor.window.onDidChangeWindowState(
      guarded(log, (state: { focused: boolean }) => {
        if (state.focused) {
          schedule({ recent: true });
        }
      }),
    ),
  );
  const tabs = editor.window.tabGroups?.onDidChangeTabs;
  if (tabs !== undefined) {
    disposables.add(tabs(() => schedule({ recent: true })));
  }

  let owner: string | null = null;
  let offered = false;
  let open = false;

  const claim = () => {
    const call = new Message({
      destination: CAMINITO,
      path: CAMINITO_PATH,
      interface: CAMINITO,
      member: 'ClaimGlobalMenu',
      signature: 'o',
      body: [MENU_PATH],
    });
    bus.call(call).catch((err: unknown) => log.line(`can't claim the menu: ${message(err)}`));
  };
  const claimIfFocused = guarded(log, () => {
    if (open && editor.window.state.focused) {
      claim();
    }
  });

  // The gate: Linux, an owner of org.mate.Caminito, and GlobalMenuOffered true.
  const update = () => {
    const now = owner !== null && offered;
    if (now === open) {
      return;
    }
    open = now;
    if (open) {
      bus.export(MENU_PATH, menu);
      claimIfFocused();
    } else {
      bus.unexport(MENU_PATH, menu);
    }
  };
  let reading = Promise.resolve();
  const readOffered = (from: string) => {
    const get = new Message({
      destination: from,
      path: CAMINITO_PATH,
      interface: PROPERTIES,
      member: 'Get',
      signature: 'ss',
      body: [CAMINITO, OFFERED],
    });
    reading = bus.call(get).then(
      (reply) => {
        if (owner === from) {
          offered = (reply?.body[0] as Variant | undefined)?.value === true;
          update();
        }
      },
      (err: unknown) => log.line(`can't read ${OFFERED}: ${message(err)}`),
    );
  };
  const setOwner = (now: string | null) => {
    if (now === owner) {
      return;
    }
    owner = now;
    offered = false;
    update();
    if (owner !== null) {
      readOffered(owner);
    }
  };

  // Signals and replies come in the order they were sent, so the last one heard is the
  // state now.
  bus.on(
    'message',
    guarded(log, (msg: Message) => {
      if (msg.type !== MessageType.SIGNAL) {
        return;
      }
      if (msg.sender === DBUS && msg.member === 'NameOwnerChanged' && msg.body[0] === CAMINITO) {
        setOwner((msg.body[2] as string) || null);
      } else if (
        msg.sender === owner &&
        msg.path === CAMINITO_PATH &&
        msg.interface === PROPERTIES &&
        msg.member === 'PropertiesChanged' &&
        msg.body[0] === CAMINITO
      ) {
        const changed = (msg.body[1] as Record<string, Variant>)[OFFERED];
        if (changed !== undefined) {
          offered = changed.value === true;
          update();
        } else if ((msg.body[2] as string[]).includes(OFFERED) && owner !== null) {
          readOffered(owner);
        }
      }
    }),
  );

  // The claim rule. Focus lost is never acted on: a panel menu taking the keyboard causes it.
  disposables.add(
    // The event's own value, since `window.state` may not have caught up with it.
    editor.window.onDidChangeWindowState(
      guarded(log, (state: { focused: boolean }) => {
        if (state.focused && open) {
          claim();
        }
      }),
    ),
  );
  disposables.add(editor.window.onDidChangeActiveTextEditor(claimIfFocused));
  for (const [, event] of lateEvents) {
    if (event !== undefined) {
      disposables.add(event(claimIfFocused));
    }
  }

  await addMatch(
    bus,
    `type='signal',sender='${DBUS}',member='NameOwnerChanged',arg0='${CAMINITO}'`,
  );
  await addMatch(
    bus,
    `type='signal',interface='${PROPERTIES}',member='PropertiesChanged',path='${CAMINITO_PATH}',arg0='${CAMINITO}'`,
  );
  // With no owner the reply is an error.
  const reply = await callDbus(bus, 'GetNameOwner', 's', [CAMINITO]).catch(() => null);
  setOwner((reply?.body[0] as string | undefined) ?? null);
  await reading;
}

/**
 * Reads the recently opened list (ADR 0020). A list that can't be read is undefined, and
 * Open Recent then has its fixed items; that it can't is logged once until it can again.
 */
class RecentReader {
  readonly #editor: Editor;
  readonly #log: Log;
  #failing = false;

  constructor(editor: Editor, log: Log) {
    this.#editor = editor;
    this.#log = log;
  }

  async read(): Promise<RecentlyOpened | undefined> {
    try {
      const list = parseRecent(await this.#editor.commands.executeCommand(GET_RECENTLY_OPENED));
      this.#failing = false;
      return list;
    } catch (err) {
      if (!this.#failing) {
        this.#log.line(
          `can't read the recently opened list, so Open Recent has its fixed items: ${message(err)}`,
        );
      }
      this.#failing = true;
      return undefined;
    }
  }
}

/** The commands the editor lists, or undefined with a line saying `otherwise` when it can't. */
async function listCommands(
  editor: Editor,
  log: Log,
  otherwise: string,
): Promise<Set<string> | undefined> {
  try {
    return new Set(await editor.commands.getCommands());
  } catch (err) {
    log.line(`can't list the commands, ${otherwise}: ${message(err)}`);
    return undefined;
  }
}

/**
 * The editor's own `product.json` (ADR 0019). One that can't be read drops the items gated
 * on it.
 */
async function productOf(editor: Editor, log: Log): Promise<Record<string, unknown>> {
  try {
    const file = join(editor.env.appRoot, 'product.json');
    return JSON.parse(await readFile(file, 'utf8')) as Record<string, unknown>;
  } catch (err) {
    log.line(
      `can't read the editor's product.json, so the items gated on it are left out: ${message(err)}`,
    );
    return {};
  }
}

/**
 * The window's settings, read with no resource or language scope (ADR 0020). A setting
 * that can't be read is undefined.
 */
function settingReader(editor: Editor): (name: string) => unknown {
  let configuration: { get(section: string): unknown } | undefined;
  try {
    configuration = editor.workspace.getConfiguration();
  } catch {
    return () => undefined;
  }
  return (name) => {
    try {
      return configuration.get(name);
    } catch {
      return undefined;
    }
  };
}

/** The window's workspace: a workspace file, else a folder, else none. */
function workspaceOf(editor: Editor): Workspace {
  if (editor.workspace.workspaceFile !== undefined) {
    return 'workspace';
  }
  return (editor.workspace.workspaceFolders?.length ?? 0) > 0 ? 'folder' : 'empty';
}

type EditorEvent = (listener: () => void) => Disposable;

/** The events of the claim rule that came late (1.67 and 1.68), with what each is for. */
function eventsNewerThanOneSixtySix(editor: Editor): [string, EditorEvent | undefined][] {
  const window = editor.window;
  return [
    ['a change of tab groups', window.tabGroups?.onDidChangeTabGroups],
    ['a change of the notebook editor', window.onDidChangeActiveNotebookEditor],
  ];
}

function addMatch(bus: MessageBus, rule: string): Promise<unknown> {
  return callDbus(bus, 'AddMatch', 's', [rule]);
}

function callDbus(bus: MessageBus, member: string, signature: string, body: unknown[]) {
  const call = { destination: DBUS, path: '/org/freedesktop/DBus', interface: DBUS };
  return bus.call(new Message({ ...call, member, signature, body }));
}

/** Whether version `a` is older than `b`, by their numeric parts. */
function olderThan(a: string, b: string): boolean {
  const parts = (version: string) =>
    version
      .split(/[.-]/)
      .slice(0, 3)
      .map((part) => parseInt(part, 10) || 0);
  const [x, y] = [parts(a), parts(b)];
  for (let i = 0; i < 3; i++) {
    if ((x[i] ?? 0) !== (y[i] ?? 0)) {
      return (x[i] ?? 0) < (y[i] ?? 0);
    }
  }
  return false;
}

/** What a stop undoes. What's added after the stop, by a start still under way, is undone at once. */
class Disposables {
  readonly #log: Log;
  #items: Disposable[] | undefined = [];

  constructor(log: Log) {
    this.#log = log;
  }

  add(disposable: Disposable): void {
    if (this.#items) {
      this.#items.push(disposable);
    } else {
      this.#dispose(disposable);
    }
  }

  stop(): void {
    const items = this.#items ?? [];
    this.#items = undefined;
    for (const disposable of items.reverse()) {
      this.#dispose(disposable);
    }
  }

  #dispose(disposable: Disposable): void {
    try {
      disposable.dispose();
    } catch (err) {
      this.#log.line(`can't stop: ${message(err)}`);
    }
  }
}

/** `f`, with what it throws logged rather than thrown into the extension host (ADR 0022). */
function guarded<A extends unknown[]>(log: Log, f: (...args: A) => void): (...args: A) => void {
  return (...args) => {
    try {
      f(...args);
    } catch (err) {
      log.line(`can't follow the editor or the bus: ${message(err)}`);
    }
  };
}

interface Log extends Disposable {
  line(text: string): void;
}

/** The extension's output channel, opened at its first line so that an idle editor shows none. */
function lazyLog(editor: Editor): Log {
  let channel: { appendLine(line: string): void; dispose(): void } | undefined;
  return {
    line(text) {
      try {
        channel ??= editor.window.createOutputChannel('Caminito Global Menu');
        channel.appendLine(text);
      } catch {
        console.error(`caminito.global-menu: ${text}`);
      }
    },
    dispose() {
      channel?.dispose();
    },
  };
}

function message(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  const text = (err as { text?: unknown } | null)?.text;
  return typeof text === 'string' ? text : String(err);
}
