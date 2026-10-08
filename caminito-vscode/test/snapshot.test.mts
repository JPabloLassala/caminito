// The developer command that makes the table (ADR 0019, 118): what it keeps of VS Code's
// `lastKnownMenubarData`, how it merges two states, and a run against a stand-in editor.

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import type { TableItem } from '../src/menu-table.mts';
import { merge, mnemonic, staleAnnotations, tableOf } from '../tools/menubar.mts';
import type { MenubarData, MenubarItem } from '../tools/menubar.mts';
import { VS_CODE } from '../src/global-menu.mts';

const SEPARATOR = { id: 'vscode.menubar.separator' };

/** File as VS Code 1.140's snapshot has it with a folder open, cut down. */
const FILE: MenubarItem[] = [
  { id: 'workbench.action.files.newUntitledFile', label: '&&New Text File' },
  {
    id: 'submenuitem.OpenProfile',
    label: 'New Window with Profile',
    submenu: {
      items: [
        { id: 'workbench.action.openProfile.Default', label: 'Default' },
        { id: 'workbench.profiles.actions.createProfile', label: 'New Profile...' },
      ],
    },
  },
  SEPARATOR,
  {
    id: 'submenuitem.MenubarRecentMenu',
    label: 'Open &&Recent',
    submenu: {
      items: [
        {
          id: 'workbench.action.reopenClosedEditor',
          label: '&&Reopen Closed Editor',
          enabled: false,
        },
        SEPARATOR,
        {
          id: 'openRecentFolder',
          label: '/tmp/a',
          uri: { scheme: 'file', path: '/tmp/a' },
          enabled: true,
        },
        SEPARATOR,
        { id: 'openRecentFile', label: '/tmp/a/x', uri: { scheme: 'file', path: '/tmp/a/x' } },
        SEPARATOR,
        { id: 'workbench.action.openRecent', label: '&&More...' },
        SEPARATOR,
        { id: 'workbench.action.clearRecentFiles', label: '&&Clear Recently Opened...' },
      ],
    },
  },
  SEPARATOR,
  {
    id: 'submenuitem.MenubarShare',
    label: 'Share',
    submenu: {
      items: [
        { id: 'workbench.profiles.actions.exportProfile', label: 'Export Profile (Default)...' },
      ],
    },
  },
  { id: 'workbench.action.toggleAutoSave', label: 'A&&uto Save', checked: true },
  SEPARATOR,
  { id: 'workbench.action.closeFolder', label: 'Close &&Folder' },
  { id: 'workbench.action.quit', label: 'E&&xit' },
];

const FOLDER: MenubarData = {
  menus: {
    File: { items: FILE },
    Help: { items: [{ id: 'workbench.action.showAboutDialog', label: '&&About' }] },
  },
  keybindings: {
    'workbench.action.quit': { label: 'Ctrl+Q', userSettingsLabel: 'ctrl+q' },
    'workbench.action.files.newUntitledFile': { label: 'Ctrl+N', userSettingsLabel: 'ctrl+n' },
  },
};

/** The same with a multi-root workspace open: Close Workspace where Close Folder was. */
const WORKSPACE: MenubarData = {
  menus: {
    File: {
      items: FILE.map((item) =>
        item.label === 'Close &&Folder' ? { ...item, label: 'Close &&Workspace' } : item,
      ),
    },
    Help: FOLDER.menus['Help']!,
  },
  keybindings: {
    'workbench.action.quit': { label: 'Ctrl+Q', userSettingsLabel: 'ctrl+q' },
    'workbench.action.files.openFolder': {
      label: 'Ctrl+K Ctrl+O',
      isNative: false,
      userSettingsLabel: 'ctrl+k ctrl+o',
    },
  },
};

const MADE = { version: '1.140.0', commit: '07f806f999227108933c2e30515b26eecc1fda74' };

test("a label's && is dbusmenu's mnemonic and an underscore is doubled", () => {
  assert.equal(mnemonic('Open Wor&&kspace from File...'), 'Open Wor_kspace from File...');
  assert.equal(mnemonic('snake_case && more'), 'snake__case _ more');
  assert.equal(mnemonic('Show All Commands'), 'Show All Commands');
});

test("the table keeps the snapshots' menus in order, merged, without what belongs to the run", () => {
  assert.deepEqual(tableOf(MADE, [FOLDER, WORKSPACE]), {
    ...MADE,
    menus: [
      {
        label: '_File',
        submenu: 'File',
        items: [
          { label: '_New Text File', command: 'workbench.action.files.newUntitledFile' },
          {
            label: 'New Window with Profile',
            submenu: 'submenuitem.OpenProfile',
            items: [
              { label: 'New Profile...', command: 'workbench.profiles.actions.createProfile' },
            ],
          },
          { separator: true },
          {
            label: 'Open _Recent',
            submenu: 'submenuitem.MenubarRecentMenu',
            items: [
              { label: '_Reopen Closed Editor', command: 'workbench.action.reopenClosedEditor' },
              { separator: true },
              { label: '_More...', command: 'workbench.action.openRecent' },
              { separator: true },
              { label: '_Clear Recently Opened...', command: 'workbench.action.clearRecentFiles' },
            ],
          },
          { separator: true },
          {
            label: 'Share',
            submenu: 'submenuitem.MenubarShare',
            items: [
              { label: 'Export Profile...', command: 'workbench.profiles.actions.exportProfile' },
            ],
          },
          { label: 'A_uto Save', command: 'workbench.action.toggleAutoSave' },
          { separator: true },
          { label: 'Close _Folder', command: 'workbench.action.closeFolder' },
          { label: 'Close _Workspace', command: 'workbench.action.closeFolder' },
          { label: 'E_xit', command: 'workbench.action.quit' },
        ],
      },
      {
        label: '_Help',
        submenu: 'Help',
        items: [{ label: '_About', command: 'workbench.action.showAboutDialog' }],
      },
    ],
    keybindings: {
      'workbench.action.files.newUntitledFile': { label: 'Ctrl+N', userSettingsLabel: 'ctrl+n' },
      'workbench.action.files.openFolder': {
        label: 'Ctrl+K Ctrl+O',
        isNative: false,
        userSettingsLabel: 'ctrl+k ctrl+o',
      },
      'workbench.action.quit': { label: 'Ctrl+Q', userSettingsLabel: 'ctrl+q' },
    },
  });
});

const a = (name: string): TableItem => ({ label: name, command: name });
const sep: TableItem = { separator: true };

test('a merge puts what only one side has where that side has it, the first side first', () => {
  assert.deepEqual(merge([a('x'), a('a1'), a('y')], [a('x'), a('b1'), a('y'), a('b2')]), [
    a('x'),
    a('a1'),
    a('b1'),
    a('y'),
    a('b2'),
  ]);
  assert.deepEqual(merge([a('x'), sep, a('y')], [a('x'), sep, a('z'), sep, a('y')]), [
    a('x'),
    sep,
    a('z'),
    sep,
    a('y'),
  ]);
  assert.deepEqual(merge([a('x'), sep], [sep, a('y')]), [a('x'), sep, a('y')]);
});

test('a merge goes into the menus both sides have', () => {
  const menu = (items: TableItem[]): TableItem => ({ label: 'M', submenu: 'm', items });
  assert.deepEqual(merge([menu([a('x')])], [menu([a('y')])]), [menu([a('x'), a('y')])]);
});

test('an annotation whose command the table lacks is stale', () => {
  const table = tableOf(MADE, [FOLDER]);
  assert.deepEqual(
    staleAnnotations(table, {
      'workbench.action.quit': {},
      'workbench.action.openRecent': { product: 'x' },
      'workbench.action.openDocumentationUrl': { product: 'documentationUrl' },
    }),
    ['workbench.action.openDocumentationUrl'],
  );
});

test("an annotation naming a label its command's items lack is stale", () => {
  const table = tableOf(MADE, [FOLDER]);
  assert.deepEqual(
    staleAnnotations(table, {
      'workbench.action.quit': { workspace: { E_xit: ['folder'], _Quit: ['workspace'] } },
    }),
    ['workbench.action.quit "_Quit"'],
  );
});

test("the committed annotations name only the committed table's commands", () => {
  assert.deepEqual(staleAnnotations(VS_CODE.table, VS_CODE.annotations), []);
});

/**
 * A stand-in for VS Code's directory: `code` writes FOLDER or WORKSPACE as
 * lastKnownMenubarData, in two steps as the real one does, and waits to be killed. The
 * second step comes 50 ms after the first and the command waits for 1000 ms without a
 * change, so it reads the whole data even on a loaded machine.
 */
function standIn(dir: string, extraHelp: MenubarItem[] = []): void {
  mkdirSync(join(dir, 'resources/app'), { recursive: true });
  writeFileSync(join(dir, 'resources/app/package.json'), JSON.stringify({ version: MADE.version }));
  writeFileSync(join(dir, 'resources/app/product.json'), JSON.stringify({ commit: MADE.commit }));
  const help = { items: [...FOLDER.menus['Help']!.items, ...extraHelp] };
  const folder = { ...FOLDER, menus: { ...FOLDER.menus, Help: help } };
  const workspace = { ...WORKSPACE, menus: { ...WORKSPACE.menus, Help: help } };
  writeFileSync(
    join(dir, 'code'),
    `#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const profile = process.argv.find((arg) => arg.startsWith('--user-data-dir=')).slice(16);
const target = process.argv[process.argv.length - 1];
const data = target.endsWith('.code-workspace') ? ${JSON.stringify(workspace)} : ${JSON.stringify(folder)};
const storage = path.join(profile, 'User/globalStorage/storage.json');
fs.mkdirSync(path.dirname(storage), { recursive: true });
const half = { ...data, menus: { File: data.menus.File } };
fs.writeFileSync(storage, JSON.stringify({ lastKnownMenubarData: half }));
setTimeout(() => fs.writeFileSync(storage, JSON.stringify({ lastKnownMenubarData: data })), 50);
setInterval(() => {}, 1000);
`,
  );
  chmodSync(join(dir, 'code'), 0o755);
}

function snapshot(editor: string, out: string, annotations: string) {
  const command = new URL('../tools/snapshot.mts', import.meta.url).pathname;
  const args = ['--editor', editor, '--out', out, '--annotations', annotations];
  return spawnSync(process.execPath, [command, ...args, '--no-xvfb', '--settle', '1000'], {
    encoding: 'utf8',
  });
}

test('the command writes the table, and running it again changes nothing', (t) => {
  const work = mkdtempSync(join(tmpdir(), 'caminito-snapshot-test-'));
  t.after(() => rmSync(work, { recursive: true, force: true }));
  standIn(join(work, 'code'), [
    { id: 'workbench.action.openDocumentationUrl', label: '&&Documentation' },
  ]);
  const annotations = join(work, 'annotations.json');
  writeFileSync(
    annotations,
    JSON.stringify({ 'workbench.action.openDocumentationUrl': { product: 'documentationUrl' } }),
  );
  const out = join(work, 'table.json');
  const first = snapshot(join(work, 'code'), out, annotations);
  assert.equal(first.status, 0, first.stderr);
  const written = readFileSync(out, 'utf8');
  const table = JSON.parse(written);
  assert.equal(table.version, MADE.version);
  assert.equal(table.commit, MADE.commit);
  assert.deepEqual(
    table.menus.map((menu: { label: string }) => menu.label),
    ['_File', '_Help'],
  );
  assert.ok(written.includes('Close _Workspace'));
  const second = snapshot(join(work, 'code'), out, annotations);
  assert.equal(second.status, 0, second.stderr);
  assert.equal(readFileSync(out, 'utf8'), written);
});

test('the command fails on an annotation whose command the snapshot lacks', (t) => {
  const work = mkdtempSync(join(tmpdir(), 'caminito-snapshot-test-'));
  t.after(() => rmSync(work, { recursive: true, force: true }));
  standIn(join(work, 'code'));
  const annotations = join(work, 'annotations.json');
  writeFileSync(
    annotations,
    JSON.stringify({ 'workbench.action.openDocumentationUrl': { product: 'documentationUrl' } }),
  );
  const out = join(work, 'table.json');
  const run = snapshot(join(work, 'code'), out, annotations);
  assert.equal(run.status, 1);
  assert.match(
    run.stderr,
    /no item of these annotated commands or labels: workbench\.action\.openDocumentationUrl/,
  );
  assert.throws(() => readFileSync(out));
});
