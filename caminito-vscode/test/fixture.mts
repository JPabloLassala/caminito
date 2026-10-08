// One extension on a private bus with a stand-in Caminito and a client, for a test.

import type { TestContext } from 'node:test';

import type { MessageBus } from '@jellybrick/dbus-next';

import { MENU_PATH, start } from '../src/global-menu.mts';
import type { GlobalMenu } from '../src/global-menu.mts';
import { Caminito, FakeEditor, PrivateBus, readMenu, until } from './harness.mts';
import type { Item, Lacking } from './harness.mts';

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
}

export interface Fixture {
  readonly bus: PrivateBus;
  readonly caminito: Caminito;
  readonly editor: FakeEditor;
  /** A connection that reads the menu, as the applet's importer does. */
  readonly client: MessageBus;
  readonly extension: GlobalMenu;
  /** The extension's unique names on the bus: one while it's connected. */
  names(): Promise<string[]>;
  /** The menu the extension serves, or undefined when it serves none. */
  menu(): Promise<Item | undefined>;
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
  const editor = new FakeEditor(options.focused ?? true);
  const extension = start(editor.api(options.version, options.lacking), {
    platform: options.platform ?? 'linux',
    busAddress: options.busAddress ?? bus.address,
  });
  t.after(() => {
    extension.dispose();
    bus.close();
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
    names,
    menu,
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
