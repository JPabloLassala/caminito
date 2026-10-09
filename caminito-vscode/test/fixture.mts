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
  /** The default keybindings' text, or what reading them fails with; none by default. */
  readonly defaultKeybindings?: string | Error;
  /** The default profile's directory, `<user data>/User`; none by default, so no user file is read. */
  readonly userDir?: string;
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
  const served = options.menu ?? VS_CODE;
  const heard = await listen(client, MENU_PATH);
  const editor = new FakeEditor(options.focused ?? true);
  editor.settings = { ...options.settings };
  editor.workspace = options.workspace ?? 'folder';
  editor.defaultKeybindings = options.defaultKeybindings ?? '[]';
  const commands = options.commands ?? commandsOf(served.table.menus);
  editor.listCommands = () =>
    commands instanceof Error ? Promise.reject(commands) : Promise.resolve([...commands]);
  const appRoot = mkdtempSync(join(tmpdir(), 'caminito-vscode-test-'));
  editor.appRoot = appRoot;
  const product = options.product === undefined ? fullProduct(served) : options.product;
  if (product !== null) {
    writeFileSync(join(appRoot, 'product.json'), JSON.stringify(product));
  }
  const extension = start(editor.api(options.version, options.lacking), {
    platform: options.platform ?? 'linux',
    busAddress: options.busAddress ?? bus.address,
    menu: served,
    ...(options.userDir !== undefined && { userDir: options.userDir }),
  });
  t.after(() => {
    extension.dispose();
    bus.close();
    rmSync(appRoot, { recursive: true, force: true });
  });
  await extension.ready;
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
  };
}
