// The global menu extension's core (ADR 0018): it serves the menu and claims it for its
// window while Caminito offers the global menu. It takes the editor's API as a value, which
// is the seam its tests use (116).

import { Message, MessageType, sessionBus } from '@jellybrick/dbus-next';
import type { MessageBus, Variant } from '@jellybrick/dbus-next';

import { DbusMenu } from './dbusmenu.mts';
import type { MenuNode } from './dbusmenu.mts';
import type { Disposable, Editor } from './editor.mts';

/** The oldest release with every stable API the extension uses, as package.json's `engines.vscode`. */
export const FLOOR = '1.68.0';

/** Where the menu is served, on the connection's unique name. */
export const MENU_PATH = '/org/mate/Caminito/GlobalMenuExtension/MenuBar';

const CAMINITO = 'org.mate.Caminito';
const CAMINITO_PATH = '/org/mate/Caminito';
const OFFERED = 'GlobalMenuOffered';
const DBUS = 'org.freedesktop.DBus';
const PROPERTIES = 'org.freedesktop.DBus.Properties';

/** The one fixed menu of this slice. Ticket 118 makes it VS Code's. */
const MENUS: readonly MenuNode[] = [
  {
    label: '_File',
    children: [{ label: '_New Text File', command: 'workbench.action.files.newUntitledFile' }],
  },
];

/** Where the extension runs. */
export interface Environment {
  readonly platform: string;
  /** The session bus's address, or `DBUS_SESSION_BUS_ADDRESS`'s when there's none. */
  readonly busAddress?: string;
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
    dispose: () => {
      disposables.stop();
      log.dispose();
    },
  };
}

async function run(
  editor: Editor,
  environment: Environment,
  log: Log,
  disposables: Disposables,
): Promise<void> {
  const floorEvents = eventsAboveTheFloor(editor);
  const missing = floorEvents.filter(([, event]) => event === undefined).map(([name]) => name);
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

  const menu = new DbusMenu(MENUS, async (command) => {
    try {
      await editor.commands.executeCommand(command);
    } catch (err) {
      log.line(`can't run ${command}: ${message(err)}`);
    }
  });

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
  const claimIfFocused = () => {
    if (open && editor.window.state.focused) {
      claim();
    }
  };

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
  bus.on('message', (msg: Message) => {
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
  });

  // The claim rule. Focus lost is never acted on: a panel menu taking the keyboard causes it.
  disposables.add(
    editor.window.onDidChangeWindowState((state) => {
      if (state.focused && open) {
        claim();
      }
    }),
  );
  disposables.add(editor.window.onDidChangeActiveTextEditor(claimIfFocused));
  for (const [, event] of floorEvents) {
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

type EditorEvent = (listener: () => void) => Disposable;

/** The events of the claim rule that came late (1.67 and 1.68), with what each is for. */
function eventsAboveTheFloor(editor: Editor): [string, EditorEvent | undefined][] {
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
export function olderThan(a: string, b: string): boolean {
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
