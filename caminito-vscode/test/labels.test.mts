// The keybinding labels a client reads, and the updates that follow the user's file, the
// window's profile and the installed extensions (ADR 0020, 120).

import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { TestContext } from 'node:test';
import { test } from 'node:test';

import { VS_CODE } from '../src/global-menu.mts';
import { commandsOf } from '../src/menu-table.mts';
import { fixture } from './fixture.mts';
import type { Fixture } from './fixture.mts';
import { until } from './harness.mts';
import type { Heard, Item } from './harness.mts';

/** Default keybindings as VS Code serves them, with its comment at the top. */
const DEFAULTS = `// Override key bindings by placing them into your key bindings file.
[
  { "key": "ctrl+s", "command": "workbench.action.files.save" },
  { "key": "ctrl+f", "command": "actions.find", "when": "editorFocus || editorIsOpen" },
  { "key": "ctrl+shift+p", "command": "workbench.action.showCommands" },
  { "key": "f1", "command": "workbench.action.showCommands" },
  { "key": "ctrl+k f", "command": "workbench.action.closeFolder", "when": "emptyWorkspaceSupport" },
]`;

const SAVE = '_File > _Save';
const CLOSE_FOLDER = '_File > Close _Folder';
const FIND = '_Edit > _Find';
const PALETTE = '_View > _Command Palette...';
const ALL_COMMANDS = '_Help > Show All Commands';
const RESET_ZOOM = '_View > _Appearance > _Reset Zoom';

/** A `<user data>/User` directory of the test's own, with `files` written in it. */
function userDir(t: TestContext, files: Record<string, unknown> = {}): string {
  const dir = mkdtempSync(join(tmpdir(), 'caminito-vscode-user-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  for (const [name, content] of Object.entries(files)) {
    write(dir, name, content);
  }
  return dir;
}

function write(dir: string, name: string, content: unknown): void {
  mkdirSync(join(dir, name, '..'), { recursive: true });
  writeFileSync(join(dir, name), typeof content === 'string' ? content : JSON.stringify(content));
}

/** Each item a client reads with a shortcut, by its labels' path. */
function shortcuts(root: Item): Record<string, unknown> {
  const found: Record<string, unknown> = {};
  const walk = (item: Item, path: string[]) => {
    const properties = Object.fromEntries(item.properties);
    const here = item.id === 0 ? path : [...path, properties['label'] as string];
    if (properties['shortcut'] !== undefined) {
      // In alphabetical order, as every item's properties are sent.
      assert.deepEqual(
        item.properties.map(([name]) => name),
        Object.keys(properties).sort(),
      );
      found[here.join(' > ')] = properties['shortcut'];
    }
    item.children.forEach((child) => walk(child, here));
  };
  walk(root, []);
  return found;
}

/** The id of the item at `path`. */
function idOf(root: Item, path: string): number {
  const walk = (item: Item, at: string[]): number | undefined => {
    const label = Object.fromEntries(item.properties)['label'] as string | undefined;
    const here = label === undefined ? at : [...at, label];
    if (here.join(' > ') === path) {
      return item.id;
    }
    return item.children.map((child) => walk(child, here)).find((id) => id !== undefined);
  };
  const id = walk(root, []);
  assert.notEqual(id, undefined, path);
  return id!;
}

/** Waits long enough that a change the extension heard would have been sent: six times its coalescing. */
function quiet(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 300));
}

/**
 * Waits until the client has heard `count` signals, then a little longer for a stray one:
 * 200 ms is four times the extension's coalescing, and a file's change reaches it in less.
 */
async function heard(f: Fixture, count: number): Promise<Heard[]> {
  await until(() => f.heard.length >= count, `${count} signals were heard`);
  await new Promise((resolve) => setTimeout(resolve, 200));
  return f.heard;
}

test("a client reads each item's shortcut, from the defaults and the user's file", async (t) => {
  const dir = userDir(t, {
    'keybindings.json': `[
      // The user's own.
      { "key": "ctrl+alt+f", "command": "actions.find" },
      { "key": "f1", "command": "-workbench.action.showCommands" },
    ]`,
  });
  const f = await fixture(t, { defaultKeybindings: DEFAULTS, userDir: dir });
  assert.deepEqual(shortcuts((await f.menu())!), {
    [SAVE]: [['Control', 's']],
    [FIND]: [['Control', 'Alt', 'f']],
    [PALETTE]: [['Control', 'Shift', 'p']],
    [ALL_COMMANDS]: [['Control', 'Shift', 'p']],
    // A chord's shortcut is each of its presses (145).
    [CLOSE_FOLDER]: [['Control', 'k'], ['f']],
  });
});

test("a change of the user's file sends one ItemsPropertiesUpdated", async (t) => {
  const dir = userDir(t);
  const f = await fixture(t, { defaultKeybindings: DEFAULTS, userDir: dir });
  const root = (await f.menu())!;
  const [save, find] = [idOf(root, SAVE), idOf(root, FIND)];

  write(dir, 'keybindings.json', [
    { key: 'ctrl+alt+f', command: 'actions.find' },
    { key: 'ctrl+s', command: '-workbench.action.files.save' },
  ]);
  assert.deepEqual(await heard(f, 1), [
    {
      signal: 'ItemsPropertiesUpdated',
      updated: [[find, [['shortcut', [['Control', 'Alt', 'f']]]]]],
      removed: [[save, ['shortcut']]],
    },
  ]);
  // The palette's last binding with no when is F1.
  assert.deepEqual(shortcuts((await f.menu())!), {
    [FIND]: [['Control', 'Alt', 'f']],
    [PALETTE]: [['F1']],
    [ALL_COMMANDS]: [['F1']],
    [CLOSE_FOLDER]: [['Control', 'k'], ['f']],
  });

  // The same bindings written again change nothing a client hears.
  write(dir, 'keybindings.json', [
    { key: 'ctrl+alt+f', command: 'actions.find' },
    { key: 'ctrl+s', command: '-workbench.action.files.save' },
  ]);
  await quiet();
  assert.equal(f.heard.length, 1);
});

test("a change of the user's file to and from a chord sends one ItemsPropertiesUpdated each", async (t) => {
  const dir = userDir(t);
  const f = await fixture(t, { defaultKeybindings: DEFAULTS, userDir: dir });
  const save = idOf((await f.menu())!, SAVE);

  write(dir, 'keybindings.json', [
    { key: 'ctrl+s', command: '-workbench.action.files.save' },
    { key: 'ctrl+k ctrl+s', command: 'workbench.action.files.save' },
  ]);
  assert.deepEqual(await heard(f, 1), [
    {
      signal: 'ItemsPropertiesUpdated',
      updated: [[save, [['shortcut', [['Control', 'k'], ['Control', 's']]]]]],
      removed: [],
    },
  ]);
  assert.deepEqual(shortcuts((await f.menu())!)[SAVE], [['Control', 'k'], ['Control', 's']]);

  write(dir, 'keybindings.json', []);
  assert.deepEqual((await heard(f, 2))[1], {
    signal: 'ItemsPropertiesUpdated',
    updated: [[save, [['shortcut', [['Control', 's']]]]]],
    removed: [],
  });
  assert.equal(f.heard.length, 2);
});

test("with the defaults unreadable, the snapshot's labels are served with the user's bindings over them", async (t) => {
  const dir = userDir(t, {
    'keybindings.json': [
      { key: 'ctrl+alt+s', command: 'workbench.action.files.save', when: 'editorFocus' },
      { key: 'ctrl+f', command: '-actions.find' },
    ],
  });
  const f = await fixture(t, {
    defaultKeybindings: new Error('no such file'),
    userDir: dir,
    workspace: 'folder',
  });
  const served = shortcuts((await f.menu())!);
  assert.deepEqual(served[SAVE], [['Control', 'Alt', 's']]);
  assert.equal(served[FIND], undefined);
  assert.deepEqual(served[PALETTE], [['Control', 'Shift', 'p']]);
  assert.deepEqual(served[RESET_ZOOM], [['Control', 'KP_0']]);
  assert.deepEqual(served[CLOSE_FOLDER], [['Control', 'k'], ['f']]);
  // Every label of the snapshot, chords among them, but the two the user's file changed.
  assert.ok(Object.keys(served).length >= Object.keys(VS_CODE.table.keybindings).length - 1);
  assert.ok(f.editor.logged.some((line) => line.startsWith("can't read the default keybindings")));
});

test("focus gained finds the window's profile again, and its file is the one watched", async (t) => {
  const profile = { location: { path: '', scheme: 'file' }, name: 'Rust' };
  const dir = userDir(t, {
    'globalStorage/storage.json': { userDataProfiles: [], profileAssociations: { workspaces: {} } },
    'keybindings.json': [{ key: 'ctrl+alt+s', command: 'workbench.action.files.save' }],
    'profiles/rust/keybindings.json': [
      { key: 'ctrl+shift+s', command: 'workbench.action.files.save' },
    ],
  });
  profile.location.path = join(dir, 'profiles/rust');
  const f = await fixture(t, { defaultKeybindings: DEFAULTS, userDir: dir });
  const save = idOf((await f.menu())!, SAVE);
  assert.deepEqual(shortcuts((await f.menu())!)[SAVE], [['Control', 'Alt', 's']]);

  // The folder, file:///a, is tied to the profile, which shows when the window is next focused.
  write(dir, 'globalStorage/storage.json', {
    userDataProfiles: [profile],
    profileAssociations: { workspaces: { 'file:///a': 'rust' } },
  });
  await quiet();
  assert.deepEqual(f.heard, []);
  f.editor.focus(true);
  assert.deepEqual(await heard(f, 1), [
    {
      signal: 'ItemsPropertiesUpdated',
      updated: [[save, [['shortcut', [['Control', 'Shift', 's']]]]]],
      removed: [],
    },
  ]);

  // The default profile's file is no longer followed, and the profile's is.
  write(dir, 'keybindings.json', [{ key: 'ctrl+1', command: 'workbench.action.files.save' }]);
  await quiet();
  assert.equal(f.heard.length, 1);
  write(dir, 'profiles/rust/keybindings.json', [
    { key: 'ctrl+2', command: 'workbench.action.files.save' },
  ]);
  const signals = await heard(f, 2);
  assert.deepEqual(signals[1], {
    signal: 'ItemsPropertiesUpdated',
    updated: [[save, [['shortcut', [['Control', '2']]]]]],
    removed: [],
  });
});

test('a change of the installed extensions reads the default keybindings again', async (t) => {
  const f = await fixture(t, { defaultKeybindings: DEFAULTS });
  const find = idOf((await f.menu())!, FIND);
  f.editor.defaultKeybindings = DEFAULTS.replace('"ctrl+f"', '"ctrl+shift+f"');
  f.editor.install(commandsOf(VS_CODE.table.menus));
  assert.deepEqual(await heard(f, 1), [
    {
      signal: 'ItemsPropertiesUpdated',
      updated: [[find, [['shortcut', [['Control', 'Shift', 'f']]]]]],
      removed: [],
    },
  ]);
  // Defaults that can't be read again keep the last read.
  f.editor.defaultKeybindings = new Error('gone');
  f.editor.install(commandsOf(VS_CODE.table.menus));
  await quiet();
  assert.equal(f.heard.length, 1);
});

test("a profile's file whose directory comes later is watched from the next focus", async (t) => {
  const dir = userDir(t);
  const location = { path: join(dir, 'profiles/rust'), scheme: 'file' };
  write(dir, 'globalStorage/storage.json', {
    userDataProfiles: [{ location, name: 'Rust' }],
    profileAssociations: { workspaces: { 'file:///a': 'rust' } },
  });
  const f = await fixture(t, { defaultKeybindings: DEFAULTS, userDir: dir });
  const save = idOf((await f.menu())!, SAVE);
  assert.equal(f.editor.logged.filter((line) => line.startsWith("can't watch")).length, 1);

  write(dir, 'profiles/rust/keybindings.json', [
    { key: 'ctrl+3', command: 'workbench.action.files.save' },
  ]);
  f.editor.focus(true);
  assert.deepEqual((await heard(f, 1))[0], {
    signal: 'ItemsPropertiesUpdated',
    updated: [[save, [['shortcut', [['Control', '3']]]]]],
    removed: [],
  });
  write(dir, 'profiles/rust/keybindings.json', [
    { key: 'ctrl+4', command: 'workbench.action.files.save' },
  ]);
  assert.deepEqual((await heard(f, 2))[1], {
    signal: 'ItemsPropertiesUpdated',
    updated: [[save, [['shortcut', [['Control', '4']]]]]],
    removed: [],
  });
});
