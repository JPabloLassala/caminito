// The weekly canary's comparison (ADR 0019, 123): the snapshot command with `--check` compares
// the structure of a fresh snapshot with the committed table, and prints the diff.

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import type { Table } from '../src/menu-table.mts';
import { structureDiff, tableOf } from '../tools/menubar.mts';
import type { Made, MenubarData, MenubarItem } from '../tools/menubar.mts';

const SEPARATOR = { id: 'vscode.menubar.separator' };

/**
 * A cut-down menubar, as a run with a folder open writes it: its throwaway profile's name is
 * in Export Profile's label, and Open Recent has a recent folder.
 */
function menubar(changes: { file?: MenubarItem[]; keybinding?: string } = {}): MenubarData {
  return {
    menus: {
      File: {
        items: [
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
                { id: 'workbench.action.reopenClosedEditor', label: '&&Reopen Closed Editor' },
                SEPARATOR,
                { id: 'openRecentFolder', label: '/tmp/a', uri: { path: '/tmp/a' } },
                SEPARATOR,
                { id: 'workbench.action.openRecent', label: '&&More...' },
              ],
            },
          },
          {
            id: 'workbench.profiles.actions.exportProfile',
            label: 'Export Profile (Default)...',
          },
          { id: 'workbench.action.toggleAutoSave', label: 'A&&uto Save' },
          ...(changes.file ?? []),
          SEPARATOR,
          { id: 'workbench.action.quit', label: 'E&&xit' },
        ],
      },
      Help: { items: [{ id: 'workbench.action.showAboutDialog', label: '&&About' }] },
    },
    keybindings: {
      'workbench.action.quit': { label: changes.keybinding ?? 'Ctrl+Q' },
    },
  };
}

const MADE: Made = { version: '1.140.0', commit: '07f806f999227108933c2e30515b26eecc1fda74' };
const NEWER: Made = { version: '1.141.0', commit: 'ffffffffffffffffffffffffffffffffffffffff' };

test("a table is the same in structure as itself, whatever its version or keybindings", () => {
  const committed = tableOf(MADE, [menubar()]);
  assert.equal(structureDiff(committed, committed), '');
  assert.equal(structureDiff(committed, tableOf(NEWER, [menubar({ keybinding: 'Ctrl+W' })])), '');
});

test('an item taken out is a difference, shown with its neighbours', () => {
  const committed = tableOf(MADE, [menubar()]);
  const file = committed.menus[0]!;
  const cut: Table = {
    ...committed,
    menus: [
      {
        ...file,
        items: file.items.filter((item) => !('command' in item) || item.label !== 'A_uto Save'),
      },
      ...committed.menus.slice(1),
    ],
  };
  const diff = structureDiff(cut, committed);
  assert.match(diff, /^--- committed table \(VS Code 1\.140\.0\)\n/);
  assert.match(diff, /^\+\+\+ snapshot \(VS Code 1\.140\.0\)\n/m);
  assert.match(diff, /\n\+  A_uto Save {2}workbench\.action\.toggleAutoSave\n/);
  assert.match(diff, /\n {3}Export Profile\.\.\. {2}workbench\.profiles\.actions\.exportProfile\n/);
  assert.doesNotMatch(diff, /^-(?!-- )/m);
});

test('an item added, a label changed and a submenu changed are differences', () => {
  const committed = tableOf(MADE, [menubar()]);
  const fresh = tableOf(NEWER, [
    menubar({ file: [{ id: 'workbench.action.files.save', label: '&&Save' }] }),
  ]);
  assert.match(structureDiff(committed, fresh), /\n\+  _Save {2}workbench\.action\.files\.save\n/);
  const renamed: Table = JSON.parse(
    JSON.stringify(committed).replace('"New Profile..."', '"New Profile…"'),
  );
  assert.match(
    structureDiff(committed, renamed),
    /\n-    New Profile\.\.\. {2}.*\n\+    New Profile… {2}/,
  );
});

test('far apart differences are separate hunks, and what lies between is left out', () => {
  const items = (n: number, changed: number[]): MenubarItem[] =>
    Array.from({ length: n }, (_, i) => ({
      id: `c${i}`,
      label: changed.includes(i) ? `changed ${i}` : `item ${i}`,
    }));
  const data = (changed: number[]): MenubarData => ({
    menus: { File: { items: items(30, changed) } },
    keybindings: {},
  });
  const diff = structureDiff(tableOf(MADE, [data([])]), tableOf(MADE, [data([2, 25])]));
  assert.equal((diff.match(/^@@$/gm) ?? []).length, 2);
  assert.doesNotMatch(diff, /item 14/);
});

/**
 * A stand-in for VS Code's directory whose `code` writes `data` as lastKnownMenubarData, for
 * either workspace state, in one write, and waits to be killed. With nothing written after
 * it, any wait reads it whole, so the check waits 500 ms rather than snapshot.test.mts's 1000.
 */
function standIn(dir: string, made: Made, data: MenubarData): void {
  mkdirSync(join(dir, 'resources/app'), { recursive: true });
  writeFileSync(join(dir, 'resources/app/package.json'), JSON.stringify({ version: made.version }));
  writeFileSync(join(dir, 'resources/app/product.json'), JSON.stringify({ commit: made.commit }));
  writeFileSync(
    join(dir, 'code'),
    `#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const profile = process.argv.find((arg) => arg.startsWith('--user-data-dir=')).slice(16);
const storage = path.join(profile, 'User/globalStorage/storage.json');
fs.mkdirSync(path.dirname(storage), { recursive: true });
fs.writeFileSync(storage, JSON.stringify({ lastKnownMenubarData: ${JSON.stringify(data)} }));
setInterval(() => {}, 1000);
`,
  );
  chmodSync(join(dir, 'code'), 0o755);
}

function check(work: string, committed: Table, made: Made, data: MenubarData) {
  standIn(join(work, 'code'), made, data);
  const table = join(work, 'table.json');
  writeFileSync(table, JSON.stringify(committed));
  const annotations = join(work, 'annotations.json');
  writeFileSync(annotations, '{}');
  const command = new URL('../tools/snapshot.mts', import.meta.url).pathname;
  const args = ['--editor', join(work, 'code'), '--out', table, '--annotations', annotations];
  const flags = ['--check', '--no-xvfb', '--settle', '500'];
  return spawnSync(process.execPath, [command, ...args, ...flags], { encoding: 'utf8' });
}

function workDir(t: { after: (f: () => void) => void }): string {
  const work = mkdtempSync(join(tmpdir(), 'caminito-canary-test-'));
  t.after(() => rmSync(work, { recursive: true, force: true }));
  return work;
}

test('the check passes against the build the table was made from', (t) => {
  const run = check(workDir(t), tableOf(MADE, [menubar()]), MADE, menubar());
  assert.equal(run.status, 0, run.stderr);
  assert.equal(run.stdout, '');
});

test('the check fails on an item taken out of the table and prints the diff', (t) => {
  const work = workDir(t);
  const committed = tableOf(MADE, [menubar()]);
  const file = committed.menus[0]!;
  const cut: Table = {
    ...committed,
    menus: [{ ...file, items: file.items.slice(1) }, ...committed.menus.slice(1)],
  };
  const run = check(work, cut, MADE, menubar());
  assert.equal(run.status, 1, run.stderr);
  assert.match(run.stdout, /\n\+  _New Text File {2}workbench\.action\.files\.newUntitledFile\n/);
  assert.deepEqual(JSON.parse(readFileSync(join(work, 'table.json'), 'utf8')), cut);
});

// The run's profile is a throwaway one, so its name is always Default's, which the committed
// table hasn't and the snapshot has.
test('what the command strips is no difference', (t) => {
  const committed = tableOf(MADE, [menubar()]);
  const fresh = menubar({ keybinding: 'Ctrl+Shift+Q' });
  const strip = (items: readonly MenubarItem[]): MenubarItem[] =>
    items.map((item) => ({
      ...item,
      enabled: false,
      checked: true,
      ...(item.submenu ? { submenu: { items: strip(item.submenu.items) } } : {}),
    }));
  const recent = (items: readonly MenubarItem[]): MenubarItem[] =>
    items.flatMap((item) =>
      item.id === 'openRecentFolder'
        ? [item, { id: 'openRecentFile', label: '/home/me/x.md', uri: { path: '/home/me/x.md' } }]
        : [item.submenu ? { ...item, submenu: { items: recent(item.submenu.items) } } : item],
    );
  const data: MenubarData = {
    ...fresh,
    menus: { ...fresh.menus, File: { items: recent(strip(fresh.menus['File']!.items)) } },
  };
  const run = check(workDir(t), committed, NEWER, data);
  assert.equal(run.status, 0, run.stdout + run.stderr);
});
