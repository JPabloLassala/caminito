// The window's keybindings file, from VS Code's `storage.json` (ADR 0020, 120).

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { keybindingsFileOf } from '../src/profiles.mts';

const USER = '/home/u/.config/Code/User';
const DEFAULT = `${USER}/keybindings.json`;

/** `storage.json` as VS Code 1.140 writes it, with a location as it stores a URI. */
const STORAGE = {
  userDataProfiles: [
    {
      location: { $mid: 1, path: `${USER}/profiles/-5a1b2c3d`, scheme: 'file' },
      name: 'Rust',
      useDefaultFlags: { settings: true },
    },
    {
      location: `file://${USER}/profiles/7e8f9a0b`,
      name: 'Writing',
      useDefaultFlags: { keybindings: true },
    },
    { location: { path: `${USER}/profiles/builtin/agents`, scheme: 'file' }, name: 'Agents' },
    // Skipped, as VS Code skips them.
    { location: { path: `${USER}/profiles/nameless`, scheme: 'file' } },
    'nonsense',
  ],
  profileAssociations: {
    workspaces: {
      'file:///home/u/rust': '-5a1b2c3d',
      'file:///home/u/both.code-workspace': '-5a1b2c3d',
      'file:///home/u/notes': '7e8f9a0b',
      'file:///home/u/plain': '__default__profile__',
      'file:///home/u/gone': 'removed-profile',
      'file:///home/u/agents.code-workspace': 'agents',
      'file:///home/u/nameless': 'nameless',
    },
    emptyWindows: { '1700000000000': '-5a1b2c3d' },
  },
};

const RUST = `${USER}/profiles/-5a1b2c3d/keybindings.json`;

test("a folder window has its folder's profile's file", () => {
  assert.equal(keybindingsFileOf(USER, STORAGE, 'file:///home/u/rust', undefined), RUST);
  assert.equal(keybindingsFileOf(USER, STORAGE, 'file:///home/u/plain', 'Rust'), DEFAULT);
});

test("a workspace-file window has its workspace file's profile's file", () => {
  assert.equal(
    keybindingsFileOf(USER, STORAGE, 'file:///home/u/both.code-workspace', undefined),
    RUST,
  );
});

test('a window with no entry has the profile window.newWindowProfile names, else the default', () => {
  for (const workspace of ['file:///home/u/elsewhere', undefined]) {
    assert.equal(keybindingsFileOf(USER, STORAGE, workspace, 'Rust'), RUST);
    assert.equal(keybindingsFileOf(USER, STORAGE, workspace, null), DEFAULT);
    assert.equal(keybindingsFileOf(USER, STORAGE, workspace, 'No Such Profile'), DEFAULT);
  }
  // An entry for a profile that's gone, or that VS Code skips, is no entry.
  assert.equal(keybindingsFileOf(USER, STORAGE, 'file:///home/u/gone', 'Rust'), RUST);
  assert.equal(keybindingsFileOf(USER, STORAGE, 'file:///home/u/nameless', undefined), DEFAULT);
});

test("a profile that uses the default's keybindings has the default profile's file", () => {
  assert.equal(keybindingsFileOf(USER, STORAGE, 'file:///home/u/notes', undefined), DEFAULT);
  assert.equal(keybindingsFileOf(USER, STORAGE, undefined, 'Writing'), DEFAULT);
  // The agents window's profile always does.
  assert.equal(
    keybindingsFileOf(USER, STORAGE, 'file:///home/u/agents.code-workspace', undefined),
    DEFAULT,
  );
});

test("an unreadable or misshapen storage.json gives the default profile's file", () => {
  const misshapen: unknown[] = [
    undefined,
    null,
    'a string',
    [STORAGE],
    { ...STORAGE, userDataProfiles: { '-5a1b2c3d': 'Rust' } },
    { ...STORAGE, profileAssociations: [] },
    { ...STORAGE, profileAssociations: { workspaces: 'file:///home/u/rust' } },
  ];
  for (const storage of misshapen) {
    assert.equal(keybindingsFileOf(USER, storage, 'file:///home/u/rust', 'Rust'), DEFAULT);
  }
  // One with no profiles yet is shaped as expected.
  assert.equal(keybindingsFileOf(USER, {}, 'file:///home/u/rust', 'Rust'), DEFAULT);
});
