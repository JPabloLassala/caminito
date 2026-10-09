// The extension's test seam (116): a private dbus-daemon, a stand-in for org.mate.Caminito
// on it, a client that reads the menu as the applet's importer does, and a fake of the
// editor's API.

import { spawn } from 'node:child_process';
import type { ChildProcess } from 'node:child_process';
import { createInterface } from 'node:readline';

import { Message, MessageType, Variant, sessionBus } from '@jellybrick/dbus-next';
import type { MessageBus } from '@jellybrick/dbus-next';

import type { Disposable, Editor } from '../src/editor.mts';
import type { Workspace } from '../src/menu-table.mts';

const DBUS = {
  destination: 'org.freedesktop.DBus',
  path: '/org/freedesktop/DBus',
  interface: 'org.freedesktop.DBus',
};

/** A dbus-daemon of the test's own. */
export class PrivateBus {
  readonly address: string;
  readonly #daemon: ChildProcess;
  readonly #connections: MessageBus[] = [];

  private constructor(address: string, daemon: ChildProcess) {
    this.address = address;
    this.#daemon = daemon;
  }

  static async start(): Promise<PrivateBus> {
    const daemon = spawn(
      'dbus-daemon',
      ['--session', '--nofork', '--nosyslog', '--print-address=1'],
      {
        stdio: ['ignore', 'pipe', 'ignore'],
      },
    );
    const lines = createInterface({ input: daemon.stdout! });
    const address = await new Promise<string>((resolve, reject) => {
      lines.once('line', resolve);
      daemon.once('error', reject);
    });
    lines.close();
    return new PrivateBus(address.trim(), daemon);
  }

  /** A new connection, closed with the bus. */
  async connect(): Promise<MessageBus> {
    const bus = sessionBus({ busAddress: this.address });
    this.#connections.push(bus);
    await call(bus, { ...DBUS, member: 'GetId' });
    return bus;
  }

  /** The unique names on the bus that aren't the test's own connections. */
  async othersThan(...ours: MessageBus[]): Promise<string[]> {
    const [bus] = this.#connections;
    const reply = await call(bus!, { ...DBUS, member: 'ListNames' });
    const names = reply.body[0] as string[];
    const own = new Set(ours.map((connection) => connection.name));
    return names.filter((name) => name.startsWith(':') && !own.has(name));
  }

  close(): void {
    for (const bus of this.#connections) {
      bus.disconnect();
    }
    this.#daemon.kill();
  }
}

/** A claim the stand-in was sent: who sent it and its path. */
interface Claim {
  readonly sender: string;
  readonly path: string;
}

/** A stand-in for Caminito's `org.mate.Caminito`, with `GlobalMenuOffered` and `ClaimGlobalMenu`. */
export class Caminito {
  readonly claims: Claim[] = [];
  #offered: boolean;
  readonly #bus: MessageBus;

  constructor(bus: MessageBus, offered: boolean) {
    this.#bus = bus;
    this.#offered = offered;
    bus.addMethodHandler((msg: Message) => this.#handle(msg));
  }

  /** Takes the name. */
  async own(): Promise<void> {
    const reply = await this.#bus.requestName('org.mate.Caminito', 0);
    if (reply !== 1) {
      throw new Error(`org.mate.Caminito wasn't granted: ${reply}`);
    }
  }

  /** Gives up the name. */
  async leave(): Promise<void> {
    await this.#bus.releaseName('org.mate.Caminito');
  }

  /** Sets `GlobalMenuOffered` and signals it. */
  offer(offered: boolean): void {
    this.#offered = offered;
    const changed = { GlobalMenuOffered: new Variant('b', offered) };
    this.#bus.send(
      Message.newSignal(
        '/org/mate/Caminito',
        'org.freedesktop.DBus.Properties',
        'PropertiesChanged',
        'sa{sv}as',
        ['org.mate.Caminito', changed, []],
      ),
    );
  }

  #handle(msg: Message): boolean {
    if (msg.path !== '/org/mate/Caminito') {
      return false;
    }
    if (msg.interface === 'org.mate.Caminito' && msg.member === 'ClaimGlobalMenu') {
      this.claims.push({ sender: msg.sender!, path: msg.body[0] as string });
      this.#bus.send(Message.newMethodReturn(msg));
      return true;
    }
    if (msg.interface === 'org.freedesktop.DBus.Properties' && msg.member === 'Get') {
      const [iface, name] = msg.body as [string, string];
      if (iface === 'org.mate.Caminito' && name === 'GlobalMenuOffered') {
        this.#bus.send(Message.newMethodReturn(msg, 'v', [new Variant('b', this.#offered)]));
        return true;
      }
    }
    return false;
  }
}

/** An item of a layout as the importer reads it, with its properties unwrapped. */
export interface Item {
  readonly id: number;
  readonly properties: [string, unknown][];
  readonly children: Item[];
}

/** Reads the menu `name` serves at `path`, as `GetLayout(0, -1, [])`. */
export async function readMenu(bus: MessageBus, name: string, path: string): Promise<Item> {
  const reply = await call(bus, {
    destination: name,
    path,
    interface: 'com.canonical.dbusmenu',
    member: 'GetLayout',
    signature: 'iias',
    body: [0, -1, []],
  });
  return item(reply.body[1] as RawItem);
}

type RawItem = [number, Record<string, Variant>, Variant[]];

function item([id, properties, children]: RawItem): Item {
  return {
    id,
    properties: Object.entries(properties).map(([key, value]) => [key, value.value]),
    children: children.map((child) => item(child.value as RawItem)),
  };
}

/** A dbusmenu signal a client heard. */
export type Heard =
  | { readonly signal: 'LayoutUpdated'; readonly revision: number; readonly parent: number }
  | {
      readonly signal: 'ItemsPropertiesUpdated';
      readonly updated: [number, [string, unknown][]][];
      readonly removed: [number, string[]][];
    };

/** Records the dbusmenu signals `bus` hears from `path`, as the importer listens for them. */
export async function listen(bus: MessageBus, path: string): Promise<Heard[]> {
  const heard: Heard[] = [];
  bus.on('message', (msg: Message) => {
    if (
      msg.type !== MessageType.SIGNAL ||
      msg.path !== path ||
      msg.interface !== 'com.canonical.dbusmenu'
    ) {
      return;
    }
    if (msg.member === 'LayoutUpdated') {
      const [revision, parent] = msg.body as [number, number];
      heard.push({ signal: 'LayoutUpdated', revision, parent });
    } else if (msg.member === 'ItemsPropertiesUpdated') {
      const [updated, removed] = msg.body as [
        [number, Record<string, Variant>][],
        [number, string[]][],
      ];
      heard.push({
        signal: 'ItemsPropertiesUpdated',
        updated: updated.map(([id, properties]) => [
          id,
          Object.entries(properties).map(([key, value]) => [key, value.value]),
        ]),
        removed,
      });
    }
  });
  await call(bus, {
    ...DBUS,
    member: 'AddMatch',
    signature: 's',
    body: [`type='signal',interface='com.canonical.dbusmenu',path='${path}'`],
  });
  return heard;
}

/** Sends the importer's `Event` for a click on `id`, and waits for the reply. */
export async function click(
  bus: MessageBus,
  name: string,
  path: string,
  id: number,
): Promise<void> {
  await call(bus, {
    destination: name,
    path,
    interface: 'com.canonical.dbusmenu',
    member: 'Event',
    signature: 'isvu',
    body: [id, 'clicked', new Variant('i', 0), 0],
  });
}

/** Calls `AboutToShow(id)` and returns its answer. */
export async function aboutToShow(
  bus: MessageBus,
  name: string,
  path: string,
  id: number,
): Promise<boolean> {
  const reply = await call(bus, {
    destination: name,
    path,
    interface: 'com.canonical.dbusmenu',
    member: 'AboutToShow',
    signature: 'i',
    body: [id],
  });
  return reply.body[0] as boolean;
}

/** Calls a method and fails on an error reply. */
export async function call(
  bus: MessageBus,
  message: ConstructorParameters<typeof Message>[0],
): Promise<Message> {
  const reply = await bus.call(new Message(message));
  if (!reply || reply.type === MessageType.ERROR) {
    throw new Error(`no reply to ${message.member}`);
  }
  return reply;
}

/** Waits until `condition` holds, for at most five seconds. */
export async function until(
  condition: () => boolean | Promise<boolean>,
  what: string,
): Promise<void> {
  const deadline = Date.now() + 5000;
  while (!(await condition())) {
    if (Date.now() > deadline) {
      throw new Error(`timed out waiting until ${what}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}

/** An event of the fake editor, fired by the test. */
class Emitter<T> {
  readonly #listeners = new Set<(value: T) => unknown>();

  readonly event = (listener: (value: T) => unknown): Disposable => {
    this.#listeners.add(listener);
    return { dispose: () => this.#listeners.delete(listener) };
  };

  fire(value: T): void {
    for (const listener of [...this.#listeners]) {
      listener(value);
    }
  }
}

/** What the fake editor lacks, to stand for one older than the floor. */
export interface Lacking {
  readonly tabGroups?: boolean;
  readonly notebooks?: boolean;
}

/** A fake of the editor's API, which records the commands run and the lines logged. */
export class FakeEditor {
  focused: boolean;
  /** The settings, read with no scope; a missing key reads as undefined. */
  settings: Record<string, unknown> = {};
  workspace: Workspace = 'folder';
  /** How many times `getCommands` was called. */
  commandListings = 0;
  /** The commands run, and what each was run with. */
  readonly commands: string[] = [];
  readonly args: unknown[][] = [];
  readonly logged: string[] = [];
  readonly windowState = new Emitter<{ focused: boolean }>();
  readonly textEditor = new Emitter<unknown>();
  readonly notebookEditor = new Emitter<unknown>();
  readonly tabGroups = new Emitter<unknown>();
  readonly tabs = new Emitter<unknown>();
  readonly configuration = new Emitter<{ affectsConfiguration(section: string): boolean }>();
  readonly workspaceFolders = new Emitter<unknown>();
  readonly extensions = new Emitter<void>();
  /** What `executeCommand` does once it has recorded the command. */
  onCommand: (command: string) => PromiseLike<unknown> = () => Promise.resolve();
  /** What `_workbench.getRecentlyOpened` answers; it isn't recorded with the commands run. */
  recent: () => PromiseLike<unknown> = () => Promise.resolve({ workspaces: [], files: [] });
  /** How many times the recent list was read. */
  recentReads = 0;
  /** What `getCommands` answers. */
  listCommands: () => PromiseLike<string[]> = () => Promise.resolve([]);
  /** `env.appRoot`, where the editor's `product.json` is. */
  appRoot = '/nonexistent';

  constructor(focused: boolean) {
    this.focused = focused;
  }

  /** Moves the focus in or out, firing the window's state event. */
  focus(focused: boolean): void {
    this.focused = focused;
    this.windowState.fire({ focused });
  }

  /** Changes settings, firing the configuration event once for them all. */
  set(changes: Record<string, unknown>): void {
    Object.assign(this.settings, changes);
    const keys = Object.keys(changes);
    this.configuration.fire({
      affectsConfiguration: (section) =>
        keys.some((key) => key === section || key.startsWith(`${section}.`)),
    });
  }

  /** Opens another workspace, firing the folders' event. */
  open(workspace: Workspace): void {
    this.workspace = workspace;
    this.workspaceFolders.fire({ added: [], removed: [] });
  }

  /** Installs or removes an extension, so that the editor lists `commands`. */
  install(commands: readonly string[]): void {
    this.listCommands = () => Promise.resolve([...commands]);
    this.extensions.fire();
  }

  /** The editor's API as the extension sees it. */
  api(version = '1.140.0', lacking: Lacking = {}): Editor {
    const fake = this;
    const window: Record<string, unknown> = {
      get state() {
        return { focused: fake.focused, active: fake.focused };
      },
      onDidChangeWindowState: this.windowState.event,
      onDidChangeActiveTextEditor: this.textEditor.event,
      createOutputChannel: () => ({
        appendLine: (line: string) => fake.logged.push(line),
        dispose: () => {},
      }),
    };
    if (!lacking.notebooks) {
      window['onDidChangeActiveNotebookEditor'] = this.notebookEditor.event;
    }
    if (!lacking.tabGroups) {
      window['tabGroups'] = {
        onDidChangeTabGroups: this.tabGroups.event,
        onDidChangeTabs: this.tabs.event,
      };
    }
    return {
      version,
      window: window as unknown as Editor['window'],
      env: {
        get appRoot() {
          return fake.appRoot;
        },
      },
      workspace: {
        get workspaceFolders() {
          const folder = (name: string, index: number) => ({ uri: `file:///${name}`, name, index });
          return {
            empty: undefined,
            folder: [folder('a', 0)],
            workspace: [folder('a', 0), folder('b', 1)],
          }[fake.workspace];
        },
        get workspaceFile() {
          return fake.workspace === 'workspace' ? 'untitled:Untitled-1.code-workspace' : undefined;
        },
        onDidChangeWorkspaceFolders: this.workspaceFolders.event,
        getConfiguration: () => ({ get: (section: string) => fake.settings[section] }),
        onDidChangeConfiguration: this.configuration.event,
      },
      extensions: { onDidChange: this.extensions.event },
      commands: {
        getCommands: () => {
          fake.commandListings++;
          return fake.listCommands();
        },
        executeCommand: (command: string, ...args: unknown[]) => {
          if (command === '_workbench.getRecentlyOpened') {
            fake.recentReads++;
            return fake.recent();
          }
          fake.commands.push(command);
          fake.args.push(args);
          return fake.onCommand(command);
        },
      },
    };
  }
}
