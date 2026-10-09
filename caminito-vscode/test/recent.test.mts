// Open Recent (ADR 0020, 121): the entries a client reads between the fixed items, what a
// click on one runs, and when the list is read again.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { MENU_PATH } from '../src/global-menu.mts';
import type { Uri } from '../src/recent.mts';
import { fixture } from './fixture.mts';
import type { Fixture } from './fixture.mts';
import { click, until } from './harness.mts';
import type { Item } from './harness.mts';

const file = (path: string): Uri => ({ scheme: 'file', authority: '', path });

const folder = (name: string) => ({ folderUri: file(`/home/ana/${name}`) });
const workspace = (name: string) => ({
  workspace: { id: name, configPath: file(`/home/ana/${name}.code-workspace`) },
});
const recentFile = (name: string) => ({ fileUri: file(`/home/ana/${name}`) });

/** Open Recent's items as a client reads them: a label, or `-` for a separator. */
async function openRecent(f: Fixture): Promise<string[]> {
  return (await recentItems(f)).map((item) => {
    const properties = Object.fromEntries(item.properties);
    return properties['type'] === 'separator' ? '-' : (properties['label'] as string);
  });
}

async function recentItems(f: Fixture): Promise<Item[]> {
  return (await recentMenu(f)).children;
}

/** Open Recent itself. */
async function recentMenu(f: Fixture): Promise<Item> {
  const label = (item: Item) => Object.fromEntries(item.properties)['label'];
  const root = (await f.menu())!;
  const fileMenu = root.children.find((item) => label(item) === '_File')!;
  return fileMenu.children.find((item) => label(item) === 'Open _Recent')!;
}

/** The id of the item labelled `label` in Open Recent. */
async function idOf(f: Fixture, label: string): Promise<number> {
  const item = (await recentItems(f)).find(
    (item) => Object.fromEntries(item.properties)['label'] === label,
  );
  assert.ok(item, label);
  return item.id;
}

async function clickOn(f: Fixture, label: string): Promise<void> {
  const [name] = await f.names();
  await click(f.client, name!, MENU_PATH, await idOf(f, label));
}

const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const FIXED = ['_Reopen Closed Editor', '-', '_More...', '-', '_Clear Recently Opened...'];

test('a client reads ten and ten in order between the fixed items, from a list of more', async (t) => {
  const workspaces = Array.from({ length: 12 }, (_, i) =>
    i % 3 === 1 ? workspace(`w${i}`) : folder(`d${i}`),
  );
  const files = Array.from({ length: 13 }, (_, i) => recentFile(`f${i}.txt`));
  const f = await fixture(t, { recent: { workspaces, files } });
  assert.deepEqual(await openRecent(f), [
    '_Reopen Closed Editor',
    '-',
    '~/d0',
    '~/w1 (Workspace)',
    '~/d2',
    '~/d3',
    '~/w4 (Workspace)',
    '~/d5',
    '~/d6',
    '~/w7 (Workspace)',
    '~/d8',
    '~/d9',
    '-',
    ...Array.from({ length: 10 }, (_, i) => `~/f${i}.txt`),
    '-',
    '_More...',
    '-',
    '_Clear Recently Opened...',
  ]);
  // An entry's label is a plain label, as the fixed items' are.
  for (const item of await recentItems(f)) {
    const names = item.properties.map(([name]) => name);
    assert.ok(names.length === 1 && (names[0] === 'label' || names[0] === 'type'));
  }
});

test('with no folders or no files, no separator is doubled', async (t) => {
  const onlyFiles = await fixture(t, { recent: { workspaces: [], files: [recentFile('a')] } });
  assert.deepEqual(await openRecent(onlyFiles), [
    '_Reopen Closed Editor',
    '-',
    '~/a',
    '-',
    '_More...',
    '-',
    '_Clear Recently Opened...',
  ]);
  const empty = await fixture(t, { recent: { workspaces: [], files: [] } });
  assert.deepEqual(await openRecent(empty), FIXED);
});

test('with the internal command failing, the fixed items are served, and that is logged once', async (t) => {
  const f = await fixture(t, {
    recent: new Error("command '_workbench.getRecentlyOpened' not found"),
  });
  assert.deepEqual(await openRecent(f), FIXED);
  assert.deepEqual(f.editor.logged, [
    "can't read the recently opened list, so Open Recent has its fixed items: command '_workbench.getRecentlyOpened' not found",
  ]);
  f.editor.focus(true);
  await until(() => f.editor.recentReads === 2, 'the list was read again');
  await pause(100);
  assert.equal(f.editor.logged.length, 1);
  assert.equal(f.heard.length, 0);
});

test("a list that isn't shaped as one is the fixed items", async (t) => {
  const f = await fixture(t, { recent: { folders: [] } });
  assert.deepEqual(await openRecent(f), FIXED);
  assert.equal(f.editor.logged.length, 1);
});

test('a click on a folder or a workspace opens it with vscode.openFolder', async (t) => {
  const f = await fixture(t, { recent: { workspaces: [folder('d'), workspace('w')], files: [] } });
  await clickOn(f, '~/d');
  await clickOn(f, '~/w (Workspace)');
  await until(() => f.editor.commands.length === 2, 'both ran');
  assert.deepEqual(f.editor.commands, ['vscode.openFolder', 'vscode.openFolder']);
  assert.deepEqual(f.editor.args, [[file('/home/ana/d')], [file('/home/ana/w.code-workspace')]]);
});

test('a click on a file opens it with _files.windowOpen', async (t) => {
  const f = await fixture(t, { recent: { workspaces: [], files: [recentFile('a.txt')] } });
  await clickOn(f, '~/a.txt');
  await until(() => f.editor.commands.length === 1, 'it ran');
  assert.deepEqual(f.editor.commands, ['_files.windowOpen']);
  assert.deepEqual(f.editor.args, [
    [[{ fileUri: file('/home/ana/a.txt') }], { remoteAuthority: null }],
  ]);
});

test('a file that _files.windowOpen fails on is opened with vscode.open', async (t) => {
  const f = await fixture(t, { recent: { workspaces: [], files: [recentFile('a.txt')] } });
  f.editor.onCommand = (command) =>
    command === '_files.windowOpen'
      ? Promise.reject(new Error("command '_files.windowOpen' not found"))
      : Promise.resolve();
  await clickOn(f, '~/a.txt');
  await until(() => f.editor.commands.length === 2, 'the fallback ran');
  assert.deepEqual(f.editor.commands, ['_files.windowOpen', 'vscode.open']);
  assert.deepEqual(f.editor.args[1], [file('/home/ana/a.txt')]);
  assert.deepEqual(f.editor.logged, [
    "can't run _files.windowOpen, so vscode.open is run: command '_files.windowOpen' not found",
  ]);
});

test('the list is read again on focus gained, and not on focus lost', async (t) => {
  const f = await fixture(t, { recent: { workspaces: [folder('a')], files: [] } });
  assert.equal(f.editor.recentReads, 1);
  f.editor.focus(false);
  await pause(200);
  assert.equal(f.editor.recentReads, 1);
  // The same list sends nothing.
  f.editor.focus(true);
  await until(() => f.editor.recentReads === 2, 'the list was read again');
  await pause(200);
  assert.equal(f.heard.length, 0);
  f.editor.recent = () => Promise.resolve({ workspaces: [folder('b'), folder('a')], files: [] });
  f.editor.focus(true);
  await until(() => f.heard.length === 1, 'an update was sent');
  // Open Recent is the menu named, since the importer fetches no other again.
  const [signal] = f.heard;
  assert.ok(signal!.signal === 'LayoutUpdated' && signal!.parent === (await recentMenu(f)).id);
  assert.deepEqual((await openRecent(f)).slice(2, 4), ['~/b', '~/a']);
});

test('the list is read again on a change of the editor tabs', async (t) => {
  const f = await fixture(t, { recent: { workspaces: [], files: [] } });
  f.editor.tabs.fire({});
  await until(() => f.editor.recentReads === 2, 'the list was read again');
  await pause(200);
  assert.equal(f.heard.length, 0);
  f.editor.recent = () => Promise.resolve({ workspaces: [], files: [recentFile('new.txt')] });
  f.editor.tabs.fire({});
  await until(() => f.heard.length === 1, 'an update was sent');
  assert.deepEqual(await openRecent(f), [
    '_Reopen Closed Editor',
    '-',
    '~/new.txt',
    '-',
    '_More...',
    '-',
    '_Clear Recently Opened...',
  ]);
});

test('the list is read again when a command run to open an entry or clear the list returns', async (t) => {
  const f = await fixture(t, {
    recent: { workspaces: [], files: [recentFile('a'), recentFile('b')] },
  });
  const a = await idOf(f, '~/a');
  const b = await idOf(f, '~/b');
  // Opening b moves it up the list, once the command returns.
  let finish: () => void = () => {};
  f.editor.onCommand = () => new Promise<void>((resolve) => (finish = resolve));
  await clickOn(f, '~/b');
  await until(() => f.editor.commands.length === 1, 'it ran');
  f.editor.recent = () =>
    Promise.resolve({ workspaces: [], files: [recentFile('b'), recentFile('a')] });
  await pause(200);
  assert.equal(f.editor.recentReads, 1);
  finish();
  await until(() => f.heard.length === 1, 'an update was sent');
  assert.deepEqual((await openRecent(f)).slice(2, 4), ['~/b', '~/a']);
  // Each entry keeps its id as it moves.
  assert.equal(await idOf(f, '~/a'), a);
  assert.equal(await idOf(f, '~/b'), b);

  f.editor.onCommand = () => Promise.resolve();
  f.editor.recent = () => Promise.resolve({ workspaces: [], files: [] });
  await clickOn(f, '_Clear Recently Opened...');
  await until(() => f.heard.length === 2, 'an update was sent');
  assert.deepEqual(f.editor.commands.at(-1), 'workbench.action.clearRecentFiles');
  assert.deepEqual(await openRecent(f), FIXED);
});

test('another command run from the menu leaves the list unread', async (t) => {
  const f = await fixture(t);
  await clickOn(f, '_Reopen Closed Editor');
  await until(() => f.editor.commands.length === 1, 'it ran');
  await pause(200);
  assert.equal(f.editor.recentReads, 1);
});

test('events that come together read the list once and send one update', async (t) => {
  const f = await fixture(t, { recent: { workspaces: [], files: [] } });
  f.editor.recent = () => Promise.resolve({ workspaces: [folder('a')], files: [recentFile('b')] });
  f.editor.focus(true);
  f.editor.tabs.fire({});
  f.editor.tabs.fire({});
  await until(() => f.heard.length === 1, 'an update was sent');
  await pause(200);
  assert.equal(f.editor.recentReads, 2);
  assert.equal(f.heard.length, 1);
});

test('an editor without tab groups still reads the list on focus gained', async (t) => {
  const f = await fixture(t, { lacking: { tabGroups: true } });
  f.editor.focus(true);
  await until(() => f.editor.recentReads === 2, 'the list was read again');
});

test('an entry the list has twice is shown once', async (t) => {
  const f = await fixture(t, {
    recent: { workspaces: [folder('a'), folder('a')], files: [recentFile('a'), recentFile('a')] },
  });
  assert.deepEqual((await openRecent(f)).slice(2, 6), ['~/a', '-', '~/a', '-']);
});
