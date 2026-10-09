// One extension on a private bus with a stand-in Caminito and a client, for a test.

import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { TestContext } from 'node:test';

import type { MessageBus } from '@jellybrick/dbus-next';

import { MENU_PATH, VS_CODE, start } from '../src/global-menu.mts';
import type { GlobalMenu, Menu } from '../src/global-menu.mts';
import { commandsOf } from '../src/menu-table.mts';
import type { Workspace } from '../src/menu-table.mts';
import { Caminito, FakeEditor, PrivateBus, call, listen, readMenu, until } from './harness.mts';
import type { Heard, Item, Lacking } from './harness.mts';

export interface Options {
  /** Whether Caminito is on the bus at activation; true by default. */
  readonly owned?: boolean;
  /** `GlobalMenuOffered` at activation; true by default. */
  readonly offered?: boolean;
  /** `window.state.focused` at activation; true by default. */
  readonly focused?: boolean;
  readonly platform?: string;
  readonly version?: string;
  readonly lacking?: Lacking;
  readonly busAddress?: string;
  /** The menu served; VS Code's by default. */
  readonly menu?: Menu;
  /** The commands the editor lists, or what listing them fails with; every command of the menu by default. */
  readonly commands?: readonly string[] | Error;
  /** The editor's `product.json`, or null for none; every annotated key set by default. */
  readonly product?: Record<string, unknown> | null;
  /** The settings at activation; none by default, so no setting can be read. */
  readonly settings?: Record<string, unknown>;
  /** The window's workspace at activation; a folder by default. */
  readonly workspace?: Workspace;
  /** The path of the window's workspace file on disk, as the agents window has; none by default. */
  readonly workspaceFile?: string;
  /** The pid of the editor's main process; `PARENT` by default. */
  readonly parentPid?: number;
}

/** The pid of the editor's main process, unless a test says otherwise. */
export const PARENT = 4242;

/** The user's settings directory, as the extension is told it. */
export const SETTINGS_DIRECTORY = '/home/user/.config/Code/User';

/** The agents window's workspace file. */
export const AGENTS_WORKSPACE = join(SETTINGS_DIRECTORY, 'agent-sessions.code-workspace');

/** Another window's core on the fixture's bus. */
export interface Core {
  readonly editor: FakeEditor;
  readonly extension: GlobalMenu;
}

/** A `product.json` with every key the menu's annotations gate on. */
function fullProduct(menu: Menu): Record<string, unknown> {
  const product: Record<string, unknown> = {};
  for (const annotation of Object.values(menu.annotations)) {
    if (annotation.product !== undefined) {
      product[annotation.product] = 'https://example.com/';
    }
  }
  return product;
}

export interface Fixture {
  readonly bus: PrivateBus;
  readonly caminito: Caminito;
  readonly editor: FakeEditor;
  /** A connection that reads the menu, as the applet's importer does. */
  readonly client: MessageBus;
  readonly extension: GlobalMenu;
  /** The dbusmenu signals the client has heard, from before activation. */
  readonly heard: Heard[];
  /** The extension's unique names on the bus: one while it's connected. */
  names(): Promise<string[]>;
  /** The menu the extension serves, or undefined when it serves none. */
  menu(): Promise<Item | undefined>;
  /** The layout's revision, as `GetLayout` answers it. */
  revision(): Promise<number>;
  /** Waits until the menu is served, or until it isn't. */
  served(served: boolean): Promise<void>;
  /** Waits until the stand-in has had `count` claims, and fails if it has had more. */
  claimed(count: number): Promise<void>;
  /** Starts another core on the bus, as another window's extension host. */
  another(options?: Options): Promise<Core>;
}

/** Starts the extension, after Caminito if it's owned, and returns once activation has been acted on. */
export async function fixture(t: TestContext, options: Options = {}): Promise<Fixture> {
  const bus = await PrivateBus.start();
  const caminitoBus = await bus.connect();
  const client = await bus.connect();
  const caminito = new Caminito(caminitoBus, options.offered ?? true);
  if (options.owned ?? true) {
    await caminito.own();
  }
  const heard = await listen(client, MENU_PATH);
  const appRoot = mkdtempSync(join(tmpdir(), 'caminito-vscode-test-'));
  const extensions: GlobalMenu[] = [];
  t.after(() => {
    for (const extension of extensions) {
      extension.dispose();
    }
    bus.close();
    rmSync(appRoot, { recursive: true, force: true });
  });
  const core = async (options: Options): Promise<Core> => {
    const menu = options.menu ?? VS_CODE;
    const editor = new FakeEditor(options.focused ?? true);
    editor.settings = { ...options.settings };
    editor.workspace = options.workspace ?? 'folder';
    editor.workspaceFile = options.workspaceFile;
    const commands = options.commands ?? commandsOf(menu.table.menus);
    editor.listCommands = () =>
      commands instanceof Error ? Promise.reject(commands) : Promise.resolve([...commands]);
    const root = mkdtempSync(join(appRoot, 'editor-'));
    editor.appRoot = root;
    const product = options.product === undefined ? fullProduct(menu) : options.product;
    if (product !== null) {
      writeFileSync(join(root, 'product.json'), JSON.stringify(product));
    }
    const extension = start(editor.api(options.version, options.lacking), {
      platform: options.platform ?? 'linux',
      busAddress: options.busAddress ?? bus.address,
      menu,
      parentPid: options.parentPid ?? PARENT,
      settingsDirectory: SETTINGS_DIRECTORY,
    });
    extensions.push(extension);
    await extension.ready;
    return { editor, extension };
  };
  const { editor, extension } = await core(options);
  const names = () => bus.othersThan(caminitoBus, client);
  const menu = async () => {
    const [name] = await names();
    return name === undefined
      ? undefined
      : readMenu(client, name, MENU_PATH).catch(() => undefined);
  };
  return {
    bus,
    caminito,
    editor,
    client,
    extension,
    heard,
    names,
    menu,
    async revision() {
      const [name] = await names();
      const reply = await call(client, {
        destination: name!,
        path: MENU_PATH,
        interface: 'com.canonical.dbusmenu',
        member: 'GetLayout',
        signature: 'iias',
        body: [0, 0, []],
      });
      return reply.body[0] as number;
    },
    async served(served) {
      const what = served ? 'the menu is served' : "the menu isn't served";
      await until(async () => ((await menu()) !== undefined) === served, what);
    },
    async claimed(count) {
      await until(() => caminito.claims.length >= count, `${count} claims were made`);
      if (caminito.claims.length !== count) {
        throw new Error(`${caminito.claims.length} claims were made, not ${count}`);
      }
    },
    another: (options = {}) => core(options),
  };
}
