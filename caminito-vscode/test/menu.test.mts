// The dbusmenu a client reads from the bus, and what a click on an item does (116, 118).

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { MENU_PATH, VS_CODE } from '../src/global-menu.mts';
import type { Menu } from '../src/global-menu.mts';
import type { TableItem } from '../src/menu-table.mts';
import { fixture } from './fixture.mts';
import { aboutToShow, click, until } from './harness.mts';
import type { Item } from './harness.mts';

/** An item as a client reads it, from its properties only. */
type Read = { label: string; children?: Read[] } | { separator: true };

function read(item: Item): Read {
  const properties = Object.fromEntries(item.properties);
  if (properties['type'] === 'separator') {
    assert.deepEqual(item.properties, [['type', 'separator']]);
    return { separator: true };
  }
  if (item.children.length === 0) {
    assert.deepEqual(item.properties, [['label', properties['label']]]);
    return { label: properties['label'] as string };
  }
  // In alphabetical order: `label` before `children-display` gives a submenu a stray `action`.
  assert.deepEqual(item.properties, [
    ['children-display', 'submenu'],
    ['label', properties['label']],
  ]);
  return { label: properties['label'] as string, children: item.children.map(read) };
}

function fromTable(item: TableItem): Read {
  if ('separator' in item) {
    return { separator: true };
  }
  if ('items' in item) {
    return { label: item.label, children: item.items.map(fromTable) };
  }
  return { label: item.label };
}

test("with every command listed, a client reads the table's menus in order", async (t) => {
  const f = await fixture(t);
  const root = (await f.menu())!;
  assert.deepEqual(root.properties, [['children-display', 'submenu']]);
  const menus = root.children.map(read);
  assert.deepEqual(menus, VS_CODE.table.menus.map(fromTable));
  assert.deepEqual(
    menus.map((menu) => 'label' in menu && menu.label),
    ['_File', '_Edit', '_Selection', '_View', '_Go', '_Run', '_Terminal', '_Help'],
  );
});

test('Open Recent has its fixed items only', async (t) => {
  const f = await fixture(t);
  const [file] = (await f.menu())!.children.map(read);
  const recent = (file as { children: Read[] }).children.find(
    (item) => 'label' in item && item.label === 'Open _Recent',
  );
  assert.deepEqual(recent, {
    label: 'Open _Recent',
    children: [
      { label: '_Reopen Closed Editor' },
      { separator: true },
      { label: '_More...' },
      { separator: true },
      { label: '_Clear Recently Opened...' },
    ],
  });
});

/** File > New Text File (2), a separator (3), Open Recent (4) > Reopen (5), Open With (6). */
const SMALL: Menu = {
  table: {
    version: '1.140.0',
    commit: '',
    keybindings: {},
    menus: [
      {
        label: '_File',
        submenu: 'File',
        items: [
          { label: '_New Text File', command: 'workbench.action.files.newUntitledFile' },
          { separator: true },
          {
            label: 'Open _Recent',
            submenu: 'submenuitem.MenubarRecentMenu',
            items: [{ label: '_Reopen', command: 'workbench.action.reopenClosedEditor' }],
          },
          { label: 'Open _With', command: 'test.openWith', args: [1, { with: 'text' }] },
        ],
      },
    ],
  },
  annotations: {},
};

test('a click is answered while its command is still running, and the command runs', async (t) => {
  const f = await fixture(t, { menu: SMALL });
  // A command that never finishes, as one that opens a modal dialog.
  f.editor.onCommand = () => new Promise(() => {});
  const [name] = await f.names();
  await click(f.client, name!, MENU_PATH, 2);
  await until(() => f.editor.commands.length === 1, 'the command ran');
  assert.deepEqual(f.editor.commands, ['workbench.action.files.newUntitledFile']);
  assert.deepEqual(f.editor.args, [[]]);
});

test('a click runs the command with the arguments the table has', async (t) => {
  const f = await fixture(t, { menu: SMALL });
  const [name] = await f.names();
  await click(f.client, name!, MENU_PATH, 6);
  await until(() => f.editor.commands.length === 1, 'the command ran');
  assert.deepEqual(f.editor.commands, ['test.openWith']);
  assert.deepEqual(f.editor.args, [[1, { with: 'text' }]]);
});

test('a click on an item of a submenu runs its command', async (t) => {
  const f = await fixture(t, { menu: SMALL });
  const [name] = await f.names();
  await click(f.client, name!, MENU_PATH, 5);
  await until(() => f.editor.commands.length === 1, 'the command ran');
  assert.deepEqual(f.editor.commands, ['workbench.action.reopenClosedEditor']);
});

test('a command that fails is logged and the click is still answered', async (t) => {
  const f = await fixture(t, { menu: SMALL });
  f.editor.onCommand = () => {
    throw new Error('no such command');
  };
  const [name] = await f.names();
  await click(f.client, name!, MENU_PATH, 2);
  await until(() => f.editor.logged.length === 1, 'the failure was logged');
  assert.match(
    f.editor.logged[0]!,
    /^can't run workbench\.action\.files\.newUntitledFile: no such command$/,
  );
});

test('a click on a submenu or a separator runs nothing', async (t) => {
  const f = await fixture(t, { menu: SMALL });
  const [name] = await f.names();
  await click(f.client, name!, MENU_PATH, 1);
  await click(f.client, name!, MENU_PATH, 3);
  await click(f.client, name!, MENU_PATH, 4);
  // The item's click comes after the others on the same connection, so it runs last if
  // they ran anything.
  await click(f.client, name!, MENU_PATH, 2);
  await until(() => f.editor.commands.length >= 1, 'the item ran');
  assert.deepEqual(f.editor.commands, ['workbench.action.files.newUntitledFile']);
});

test('AboutToShow is answered with false', async (t) => {
  const f = await fixture(t, { menu: SMALL });
  const [name] = await f.names();
  assert.equal(await aboutToShow(f.client, name!, MENU_PATH, 1), false);
});
