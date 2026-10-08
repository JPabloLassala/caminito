// The state the menu mirrors and the updates that follow it (ADR 0020, 119): a checkmark
// where an item mirrors a setting, the items that follow the workspace, and what a client
// hears when either changes or an extension adds a command.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { MENU_PATH, VS_CODE } from '../src/global-menu.mts';
import type { Menu } from '../src/global-menu.mts';
import { commandsOf } from '../src/menu-table.mts';
import { fixture } from './fixture.mts';
import type { Fixture } from './fixture.mts';
import { aboutToShow, until } from './harness.mts';
import type { Heard, Item } from './harness.mts';

/** VS Code's defaults for the twelve annotated settings. */
const DEFAULTS = {
  'files.autoSave': 'off',
  'editor.columnSelection': false,
  'workbench.statusBar.visible': true,
  'editor.minimap.enabled': true,
  'breadcrumbs.enabled': true,
  'editor.stickyScroll.enabled': true,
  'editor.renderControlCharacters': true,
  'window.menuBarVisibility': 'classic',
  'editor.renderWhitespace': 'selection',
  'workbench.activityBar.location': 'default',
  'workbench.editor.showTabs': 'multiple',
  'workbench.editor.editorActionsLocation': 'default',
};

/** Each item a client reads with a toggle, by its labels' path, as "checkmark on" and so on. */
function toggles(root: Item): Record<string, string> {
  const found: Record<string, string> = {};
  const walk = (item: Item, path: string[]) => {
    const properties = Object.fromEntries(item.properties);
    const here = item.id === 0 ? path : [...path, properties['label'] as string];
    if (properties['toggle-type'] !== undefined) {
      // In alphabetical order, as every item's properties are sent.
      assert.deepEqual(
        item.properties.map(([name]) => name),
        Object.keys(properties).sort(),
      );
      const state = properties['toggle-state'] === 1 ? 'on' : 'off';
      found[here.join(' > ')] = `${properties['toggle-type']} ${state}`;
    }
    for (const child of item.children) {
      walk(child, here);
    }
  };
  walk(root, []);
  return found;
}

/** The labels of every item a client reads, by their path. */
function labels(root: Item): string[] {
  const found: string[] = [];
  const walk = (item: Item, path: string[]) => {
    const label = Object.fromEntries(item.properties)['label'] as string | undefined;
    const here = label === undefined ? path : [...path, label];
    if (label !== undefined) {
      found.push(here.join(' > '));
    }
    for (const child of item.children) {
      walk(child, here);
    }
  };
  walk(root, []);
  return found;
}

async function read(f: Fixture): Promise<Item> {
  return (await f.menu())!;
}

const APPEARANCE = '_View > _Appearance';

test('each of the twelve reads its setting, and a radio group has one item on', async (t) => {
  const f = await fixture(t, { settings: DEFAULTS });
  assert.deepEqual(toggles(await read(f)), {
    '_File > A_uto Save': 'checkmark off',
    '_Selection > Column _Selection Mode': 'checkmark off',
    [`${APPEARANCE} > Menu _Bar`]: 'checkmark on',
    [`${APPEARANCE} > S_tatus Bar`]: 'checkmark on',
    [`${APPEARANCE} > Activity Bar Position > _Default`]: 'radio on',
    [`${APPEARANCE} > Activity Bar Position > _Top`]: 'radio off',
    [`${APPEARANCE} > Activity Bar Position > _Bottom`]: 'radio off',
    [`${APPEARANCE} > Activity Bar Position > _Hidden`]: 'radio off',
    [`${APPEARANCE} > Tab Bar > Multiple Tabs`]: 'radio on',
    [`${APPEARANCE} > Tab Bar > Single Tab`]: 'radio off',
    [`${APPEARANCE} > Tab Bar > Hidden`]: 'radio off',
    [`${APPEARANCE} > Editor Actions Position > Tab Bar`]: 'radio on',
    [`${APPEARANCE} > Editor Actions Position > Title Bar`]: 'radio off',
    [`${APPEARANCE} > Editor Actions Position > Hidden`]: 'radio off',
    [`${APPEARANCE} > _Minimap`]: 'checkmark on',
    [`${APPEARANCE} > _Breadcrumbs`]: 'checkmark on',
    [`${APPEARANCE} > _Sticky Scroll`]: 'checkmark on',
    [`${APPEARANCE} > Render Whitespace > Set Render Whitespace to None`]: 'radio off',
    [`${APPEARANCE} > Render Whitespace > Set Render Whitespace to Boundary`]: 'radio off',
    [`${APPEARANCE} > Render Whitespace > Set Render Whitespace to Selection`]: 'radio on',
    [`${APPEARANCE} > Render Whitespace > Set Render Whitespace to Trailing`]: 'radio off',
    [`${APPEARANCE} > Render Whitespace > Set Render Whitespace to All`]: 'radio off',
    [`${APPEARANCE} > Render _Control Characters`]: 'checkmark on',
  });
});

test('each of the twelve follows its setting away from the default', async (t) => {
  const f = await fixture(t, {
    settings: {
      'files.autoSave': 'afterDelay',
      'editor.columnSelection': true,
      'workbench.statusBar.visible': false,
      'editor.minimap.enabled': false,
      'breadcrumbs.enabled': false,
      'editor.stickyScroll.enabled': false,
      'editor.renderControlCharacters': false,
      'window.menuBarVisibility': 'toggle',
      'editor.renderWhitespace': 'all',
      'workbench.activityBar.location': 'hidden',
      'workbench.editor.showTabs': 'single',
      'workbench.editor.editorActionsLocation': 'hidden',
    },
  });
  const on = Object.entries(toggles(await read(f)))
    .filter(([, state]) => state.endsWith(' on'))
    .map(([path]) => path);
  assert.deepEqual(on, [
    '_File > A_uto Save',
    '_Selection > Column _Selection Mode',
    `${APPEARANCE} > Activity Bar Position > _Hidden`,
    `${APPEARANCE} > Tab Bar > Single Tab`,
    `${APPEARANCE} > Editor Actions Position > Hidden`,
    `${APPEARANCE} > Render Whitespace > Set Render Whitespace to All`,
  ]);
});

test('Menu Bar is on when the setting is classic or visible', async (t) => {
  for (const [value, state] of [
    ['classic', 'on'],
    ['visible', 'on'],
    ['toggle', 'off'],
    ['compact', 'off'],
    ['hidden', 'off'],
  ]) {
    const f = await fixture(t, { settings: { 'window.menuBarVisibility': value } });
    assert.equal(toggles(await read(f))[`${APPEARANCE} > Menu _Bar`], `checkmark ${state}`, value);
  }
});

test("Editor Actions Position's Title Bar is on when the tabs are hidden and the actions' place is the default", async (t) => {
  const f = await fixture(t, {
    settings: {
      'workbench.editor.showTabs': 'none',
      'workbench.editor.editorActionsLocation': 'default',
    },
  });
  const read_ = toggles(await read(f));
  const position = `${APPEARANCE} > Editor Actions Position`;
  assert.deepEqual(
    [`${position} > Tab Bar`, `${position} > Title Bar`, `${position} > Hidden`].map(
      (path) => read_[path],
    ),
    ['radio off', 'radio on', 'radio off'],
  );
});

test("an item whose setting can't be read is a plain item", async (t) => {
  const f = await fixture(t, {
    settings: { 'editor.minimap.enabled': 'yes', 'workbench.editor.showTabs': 'none' },
  });
  // Minimap's value isn't a boolean, and Title Bar's other setting is missing.
  assert.deepEqual(toggles(await read(f)), {
    [`${APPEARANCE} > Tab Bar > Multiple Tabs`]: 'radio off',
    [`${APPEARANCE} > Tab Bar > Single Tab`]: 'radio off',
    [`${APPEARANCE} > Tab Bar > Hidden`]: 'radio on',
  });
});

/** The id of the item at `path`, as a client reads it. */
function idOf(root: Item, path: string): number {
  const walk = (item: Item, here: string[]): number | undefined => {
    const label = Object.fromEntries(item.properties)['label'] as string | undefined;
    const next = label === undefined ? here : [...here, label];
    if (next.join(' > ') === path) {
      return item.id;
    }
    for (const child of item.children) {
      const found = walk(child, next);
      if (found !== undefined) {
        return found;
      }
    }
    return undefined;
  };
  const id = walk(root, []);
  assert.notEqual(id, undefined, path);
  return id!;
}

/** Waits until the client has heard `count` signals, then a little longer for a stray one. */
async function heard(f: Fixture, count: number): Promise<Heard[]> {
  await until(() => f.heard.length >= count, `${count} signals were heard`);
  await new Promise((resolve) => setTimeout(resolve, 200));
  return f.heard;
}

test('a change of an annotated setting sends one ItemsPropertiesUpdated', async (t) => {
  const f = await fixture(t, { settings: DEFAULTS });
  const minimap = idOf(await read(f), `${APPEARANCE} > _Minimap`);
  const revision = await f.revision();
  f.editor.set({ 'editor.minimap.enabled': false });
  assert.deepEqual(await heard(f, 1), [
    { signal: 'ItemsPropertiesUpdated', updated: [[minimap, [['toggle-state', 0]]]], removed: [] },
  ]);
  assert.equal(toggles(await read(f))[`${APPEARANCE} > _Minimap`], 'checkmark off');
  assert.equal(await f.revision(), revision);
});

test('a change that leaves the menu as it was sends nothing', async (t) => {
  const f = await fixture(t, { settings: DEFAULTS });
  const autoSave = idOf(await read(f), '_File > A_uto Save');
  // Still not off, and a setting no item follows.
  f.editor.set({ 'files.autoSave': 'afterDelay' });
  f.editor.set({ 'editor.fontSize': 20 });
  await new Promise((resolve) => setTimeout(resolve, 200));
  // The first change already checked Auto Save; this one leaves it checked.
  f.editor.set({ 'files.autoSave': 'onFocusChange' });
  f.editor.set({ 'editor.fontSize': 21 });
  // Hand the menu something to send, so a stray signal would come before it.
  f.editor.set({ 'editor.minimap.enabled': false });
  const signals = await heard(f, 2);
  assert.equal(signals.length, 2);
  assert.deepEqual(signals[0], {
    signal: 'ItemsPropertiesUpdated',
    updated: [[autoSave, [['toggle-state', 1]]]],
    removed: [],
  });
});

test("a setting that can no longer be read takes the item's toggle away", async (t) => {
  const f = await fixture(t, { settings: DEFAULTS });
  const minimap = idOf(await read(f), `${APPEARANCE} > _Minimap`);
  f.editor.set({ 'editor.minimap.enabled': undefined });
  assert.deepEqual(await heard(f, 1), [
    {
      signal: 'ItemsPropertiesUpdated',
      updated: [],
      removed: [[minimap, ['toggle-state', 'toggle-type']]],
    },
  ]);
});

test('several changes at once are sent as one update', async (t) => {
  const f = await fixture(t, { settings: DEFAULTS });
  const root = await read(f);
  const minimap = idOf(root, `${APPEARANCE} > _Minimap`);
  const autoSave = idOf(root, '_File > A_uto Save');
  f.editor.set({ 'editor.minimap.enabled': false });
  f.editor.set({ 'files.autoSave': 'afterDelay' });
  assert.deepEqual(await heard(f, 1), [
    {
      signal: 'ItemsPropertiesUpdated',
      updated: [
        [autoSave, [['toggle-state', 1]]],
        [minimap, [['toggle-state', 0]]],
      ],
      removed: [],
    },
  ]);
});

const CLOSE_FOLDER = '_File > Close _Folder';
const CLOSE_WORKSPACE = '_File > Close _Workspace';

test("a window's workspace decides between Close Folder and Close Workspace", async (t) => {
  for (const [workspace, shown] of [
    ['empty', []],
    ['folder', [CLOSE_FOLDER]],
    ['workspace', [CLOSE_WORKSPACE]],
  ] as const) {
    const f = await fixture(t, { workspace });
    const closes = labels(await read(f)).filter((path) => path.startsWith('_File > Close _'));
    assert.deepEqual(closes, shown, workspace);
  }
});

test('adding a second folder shows the multi-root items with one LayoutUpdated, whose revision grows', async (t) => {
  const f = await fixture(t, { workspace: 'folder' });
  const revision = await f.revision();
  f.editor.open('workspace');
  const signals = await heard(f, 1);
  assert.equal(signals.length, 1);
  const [signal] = signals;
  assert.equal(signal!.signal, 'LayoutUpdated');
  assert.ok(signal!.signal === 'LayoutUpdated' && signal!.revision > revision);
  assert.equal(signal!.signal === 'LayoutUpdated' && signal!.parent, 0);
  assert.equal(await f.revision(), signal!.signal === 'LayoutUpdated' && signal!.revision);
  const shown = labels(await read(f));
  assert.ok(shown.includes(CLOSE_WORKSPACE));
  assert.ok(!shown.includes(CLOSE_FOLDER));
});

test('an item keeps its id when the layout changes around it', async (t) => {
  const f = await fixture(t, { workspace: 'folder' });
  const before = await read(f);
  f.editor.open('workspace');
  await heard(f, 1);
  const after = await read(f);
  assert.equal(idOf(after, '_File > Clos_e Window'), idOf(before, '_File > Clos_e Window'));
  assert.equal(idOf(after, '_Help > _About'), idOf(before, '_Help > _About'));
});

test('a change of structure and of a checkmark at once is one LayoutUpdated', async (t) => {
  const f = await fixture(t, { settings: DEFAULTS, workspace: 'folder' });
  f.editor.set({ 'editor.minimap.enabled': false });
  f.editor.open('workspace');
  const signals = await heard(f, 1);
  assert.deepEqual(
    signals.map((signal) => signal.signal),
    ['LayoutUpdated'],
  );
  assert.equal(toggles(await read(f))[`${APPEARANCE} > _Minimap`], 'checkmark off');
});

/** File > New (1), a command another extension adds (2). */
const ADDED: Menu = {
  table: {
    version: '1.140.0',
    commit: '',
    keybindings: {},
    menus: [
      {
        label: '_File',
        submenu: 'File',
        items: [
          { label: '_New', command: 'test.new' },
          { label: '_Added', command: 'test.added' },
        ],
      },
    ],
  },
  annotations: {},
};

test('installing an extension that adds a command the table names brings its item back', async (t) => {
  const f = await fixture(t, { menu: ADDED, commands: ['test.new'] });
  assert.deepEqual(labels(await read(f)), ['_File', '_File > _New']);
  const revision = await f.revision();
  f.editor.install(['test.new', 'test.added']);
  const signals = await heard(f, 1);
  assert.deepEqual(
    signals.map((signal) => signal.signal),
    ['LayoutUpdated'],
  );
  assert.ok(signals[0]!.signal === 'LayoutUpdated' && signals[0]!.revision > revision);
  assert.deepEqual(labels(await read(f)), ['_File', '_File > _New', '_File > _Added']);
});

test('a change of extensions that adds no command of the table sends nothing', async (t) => {
  const f = await fixture(t, { settings: DEFAULTS });
  f.editor.install([...commandsOf(VS_CODE.table.menus), 'other.extension.command']);
  await until(() => f.editor.commandListings === 2, 'the commands were listed again');
  f.editor.set({ 'editor.minimap.enabled': false });
  const signals = await heard(f, 1);
  assert.deepEqual(
    signals.map((signal) => signal.signal),
    ['ItemsPropertiesUpdated'],
  );
});

test("nothing is re-read on AboutToShow, and nothing is sent while nothing's changed", async (t) => {
  const f = await fixture(t, { settings: DEFAULTS });
  const [name] = await f.names();
  // A setting changed with no event: only an event makes the menu re-read.
  f.editor.settings['editor.minimap.enabled'] = false;
  const appearance = idOf(await read(f), APPEARANCE);
  assert.equal(await aboutToShow(f.client, name!, MENU_PATH, appearance), false);
  await new Promise((resolve) => setTimeout(resolve, 300));
  assert.deepEqual(f.heard, []);
  assert.equal(toggles(await read(f))[`${APPEARANCE} > _Minimap`], 'checkmark on');
  assert.equal(f.editor.commandListings, 1);
});
