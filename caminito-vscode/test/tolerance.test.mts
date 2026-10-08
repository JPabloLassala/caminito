// The table fitted to the running editor, as a client reads it (ADR 0019, 118): an item whose
// command is missing is dropped, then a menu left empty, then the separators collapse, and
// an item gated on a product.json key is there only when the editor's product.json has it.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import type { Menu } from '../src/global-menu.mts';
import type { TableItem, TableSubmenu } from '../src/menu-table.mts';
import { commandsOf } from '../src/menu-table.mts';
import { fixture } from './fixture.mts';
import type { Fixture } from './fixture.mts';
import type { Item } from './harness.mts';

const item = (name: string): TableItem => ({ label: name, command: `test.${name}` });
const separator: TableItem = { separator: true };
const submenu = (name: string, items: TableItem[]): TableSubmenu => ({
  label: name,
  submenu: `test.${name}`,
  items,
});

function menu(menus: TableSubmenu[], annotations: Menu['annotations'] = {}): Menu {
  return { table: { version: '1.140.0', commit: '', keybindings: {}, menus }, annotations };
}

/** The menu as a client reads it: a label, a list for a submenu, '-' for a separator. */
type Shown = string | '-' | { [label: string]: Shown[] };

function shown(item: Item): Shown {
  const properties = Object.fromEntries(item.properties);
  if (properties['type'] === 'separator') {
    return '-';
  }
  const label = properties['label'] as string;
  return item.children.length === 0 ? label : { [label]: item.children.map(shown) };
}

async function read(f: Fixture): Promise<Shown[]> {
  return (await f.menu())!.children.map(shown);
}

const MIXED = menu([
  submenu('A', [
    separator,
    item('gone1'),
    separator,
    item('kept1'),
    separator,
    separator,
    submenu('Empty', [item('gone2'), separator, item('gone3')]),
    separator,
    submenu('Half', [separator, item('gone4'), separator, item('kept2'), separator]),
    item('gone5'),
    separator,
    item('kept3'),
    separator,
    item('gone6'),
  ]),
  submenu('B', [item('gone7'), separator, item('gone8')]),
  submenu('C', [item('kept4')]),
]);

test('with every command listed, every item and separator is read', async (t) => {
  const f = await fixture(t, { menu: menu([submenu('A', [item('x'), separator, item('y')])]) });
  assert.deepEqual(await read(f), [{ A: ['x', '-', 'y'] }]);
});

test('an item whose command is missing is gone, then a menu left empty, and no separator is at an end or beside another', async (t) => {
  const listed = commandsOf(MIXED.table.menus).filter(
    (command) => !command.startsWith('test.gone'),
  );
  const f = await fixture(t, { menu: MIXED, commands: listed });
  assert.deepEqual(await read(f), [
    { A: ['kept1', '-', { Half: ['kept2'] }, '-', 'kept3'] },
    { C: ['kept4'] },
  ]);
});

const GATED = menu([submenu('Help', [item('docs'), separator, item('about')])], {
  'test.docs': { product: 'documentationUrl' },
});

test("an item gated on a product.json key is there when the editor's product.json has it", async (t) => {
  const f = await fixture(t, {
    menu: GATED,
    product: { documentationUrl: 'https://code.visualstudio.com/docs' },
  });
  assert.deepEqual(await read(f), [{ Help: ['docs', '-', 'about'] }]);
});

test("an item gated on a product.json key is gone when the editor's product.json lacks it", async (t) => {
  const f = await fixture(t, { menu: GATED, product: { nameShort: 'Code - OSS' } });
  assert.deepEqual(await read(f), [{ Help: ['about'] }]);
  assert.deepEqual(f.editor.logged, []);
});

test("without a product.json the gated items are gone, and that's logged", async (t) => {
  const f = await fixture(t, { menu: GATED, product: null });
  assert.deepEqual(await read(f), [{ Help: ['about'] }]);
  assert.equal(f.editor.logged.length, 1);
  assert.match(
    f.editor.logged[0]!,
    /^can't read the editor's product\.json, so the items gated on it are left out: /,
  );
});

test("when the commands can't be listed every item is kept, and that's logged", async (t) => {
  const f = await fixture(t, {
    menu: menu([submenu('A', [item('x'), separator, item('y')])]),
    commands: new Error('no commands'),
  });
  assert.deepEqual(await read(f), [{ A: ['x', '-', 'y'] }]);
  assert.deepEqual(f.editor.logged, [
    "can't list the commands, so every item is kept: no commands",
  ]);
});

test("one line names the table's version and the editor's when they differ", async (t) => {
  const f = await fixture(t, { version: '1.141.0' });
  await f.claimed(1);
  assert.deepEqual(f.editor.logged, [
    "the menu table is VS Code 1.140.0's and the editor is 1.141.0",
  ]);
});

test("nothing is logged when the editor is the table's version", async (t) => {
  const f = await fixture(t, { version: '1.140.0' });
  await f.claimed(1);
  assert.deepEqual(f.editor.logged, []);
});
