// A recent entry's label, as a function of the entry (ADR 0020, 121).

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { MAX_LABEL, parseRecent, recentLabel } from '../src/recent.mts';
import type { Uri } from '../src/recent.mts';

const HOME = '/home/ana';

const file = (path: string): Uri => ({ scheme: 'file', authority: '', path });
const remote = (authority: string, path: string): Uri => ({
  scheme: 'vscode-remote',
  authority,
  path,
});

test('a path under home has the home directory as ~', () => {
  assert.equal(recentLabel({ folderUri: file('/home/ana/src/camino') }, HOME), '~/src/camino');
  assert.equal(recentLabel({ fileUri: file('/home/ana/notes.txt') }, HOME), '~/notes.txt');
  assert.equal(recentLabel({ folderUri: file('/home/ana') }, HOME), '~');
  assert.equal(recentLabel({ folderUri: file('/home/ana/') }, `${HOME}/`), '~/');
});

test('a path elsewhere is shown as it is', () => {
  assert.equal(recentLabel({ folderUri: file('/srv/www') }, HOME), '/srv/www');
  // Another user whose name begins with this one's.
  assert.equal(recentLabel({ folderUri: file('/home/anabel/x') }, HOME), '/home/anabel/x');
});

test('a workspace file is its path less the extension, marked (Workspace)', () => {
  assert.equal(
    recentLabel({ workspace: { configPath: file('/home/ana/w/camino.code-workspace') } }, HOME),
    '~/w/camino (Workspace)',
  );
  assert.equal(
    recentLabel({ workspace: { configPath: file('/srv/other.json') } }, HOME),
    '/srv/other.json (Workspace)',
  );
});

test('a remote is marked with its kind and host, and its path is left as it is', () => {
  assert.equal(
    recentLabel(
      { folderUri: remote('ssh-remote+box', '/home/ana/src'), remoteAuthority: 'ssh-remote+box' },
      HOME,
    ),
    '/home/ana/src [SSH: box]',
  );
  assert.equal(
    recentLabel({ fileUri: remote('wsl+Ubuntu', '/etc/hosts') }, HOME),
    '/etc/hosts [WSL: Ubuntu]',
  );
  assert.equal(
    recentLabel(
      { workspace: { configPath: remote('ssh-remote+box', '/w/a.code-workspace') } },
      HOME,
    ),
    '/w/a (Workspace) [SSH: box]',
  );
  assert.equal(
    recentLabel({ folderUri: remote('dev-container+7b2268', '/workspaces/x') }, HOME),
    '/workspaces/x [Dev Container]',
  );
  assert.equal(
    recentLabel({ folderUri: remote('some-remote+thing', '/x') }, HOME),
    '/x [some-remote+thing]',
  );
});

test('another scheme is shown as its URI', () => {
  assert.equal(
    recentLabel({ folderUri: { scheme: 'vscode-vfs', authority: 'github', path: '/a/b' } }, HOME),
    'vscode-vfs://github/a/b',
  );
});

test('a long path is cut in the middle at 120 characters', () => {
  const long = `/srv/${'a'.repeat(100)}/${'b'.repeat(100)}.txt`;
  const label = recentLabel({ fileUri: file(long) }, HOME);
  assert.equal(label.length, MAX_LABEL);
  assert.equal(label, `${long.slice(0, 59)}…${long.slice(-60)}`);
  const exact = `/${'c'.repeat(MAX_LABEL - 1)}`;
  assert.equal(recentLabel({ fileUri: file(exact) }, HOME), exact);
});

test('each underscore is doubled, after the cut', () => {
  assert.equal(recentLabel({ fileUri: file('/home/ana/my_file__x') }, HOME), '~/my__file____x');
  const long = `/${'_'.repeat(200)}`;
  const label = recentLabel({ fileUri: file(long) }, HOME);
  assert.equal(label, `/${'__'.repeat(58)}…${'__'.repeat(60)}`);
});

test("an entry's own label is shown as it is, cut and with its underscores doubled", () => {
  assert.equal(
    recentLabel({ folderUri: file('/home/ana/x'), label: 'my_project' }, HOME),
    'my__project',
  );
});

test("a list that isn't shaped as one throws, and an entry that isn't is left out", () => {
  assert.throws(() => parseRecent(undefined));
  assert.throws(() => parseRecent({ workspaces: [] }));
  const folder = { folderUri: file('/a') };
  const workspace = { workspace: { id: 'x', configPath: file('/w.code-workspace') } };
  const one = { fileUri: file('/f') };
  assert.deepEqual(
    parseRecent({
      workspaces: [folder, { folderUri: '/b' }, null, workspace],
      files: [one, { fileUri: { scheme: 'file' } }],
    }),
    { workspaces: [folder, workspace], files: [one] },
  );
});
