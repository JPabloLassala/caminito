// The dbusmenu a client reads from the bus, and what a click on its item does (116).

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { MENU_PATH } from '../src/global-menu.mts';
import { fixture } from './fixture.mts';
import { aboutToShow, click, until } from './harness.mts';

test('a client reads the whole menu with its properties in alphabetical order', async (t) => {
  const f = await fixture(t);
  assert.deepEqual(await f.menu(), {
    id: 0,
    properties: [['children-display', 'submenu']],
    children: [
      {
        id: 1,
        properties: [
          ['children-display', 'submenu'],
          ['label', '_File'],
        ],
        children: [{ id: 2, properties: [['label', '_New Text File']], children: [] }],
      },
    ],
  });
});

test('a click is answered while its command is still running, and the command runs', async (t) => {
  const f = await fixture(t);
  // A command that never finishes, as one that opens a modal dialog.
  f.editor.onCommand = () => new Promise(() => {});
  const [name] = await f.names();
  await click(f.client, name!, MENU_PATH, 2);
  await until(() => f.editor.commands.length === 1, 'the command ran');
  assert.deepEqual(f.editor.commands, ['workbench.action.files.newUntitledFile']);
});

test('a command that fails is logged and the click is still answered', async (t) => {
  const f = await fixture(t);
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

test('a click on a submenu runs nothing', async (t) => {
  const f = await fixture(t);
  const [name] = await f.names();
  await click(f.client, name!, MENU_PATH, 1);
  // The item's click comes after the submenu's on the same connection, so it runs first if
  // the submenu's ran anything.
  await click(f.client, name!, MENU_PATH, 2);
  await until(() => f.editor.commands.length >= 1, 'the item ran');
  assert.deepEqual(f.editor.commands, ['workbench.action.files.newUntitledFile']);
});

test('AboutToShow is answered with false', async (t) => {
  const f = await fixture(t);
  const [name] = await f.names();
  assert.equal(await aboutToShow(f.client, name!, MENU_PATH, 1), false);
});
