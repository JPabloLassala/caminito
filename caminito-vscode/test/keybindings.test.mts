// The label rule and the key names, as functions of the two files (ADR 0020, 120).

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { VS_CODE } from '../src/global-menu.mts';
import {
  GDK_KEYS,
  WITHOUT_GDK_KEY,
  bindingsOf,
  fallbackLabelsOf,
  labelsOf,
  parseJsonc,
  shortcutOf,
} from '../src/keybindings.mts';
import type { Binding } from '../src/keybindings.mts';

/** The shortcut each command gets from the defaults and the user's file. */
function shortcuts(defaults: Binding[], user: Binding[] = []) {
  return Object.fromEntries([...labelsOf(defaults, user)].map(([c, key]) => [c, shortcutOf(key)]));
}

test('a command with one binding has it, whatever its when', () => {
  assert.deepEqual(
    shortcuts([
      { key: 'ctrl+shift+p', command: 'palette' },
      { key: 'f2', command: 'rename', when: 'editorHasRenameProvider' },
    ]),
    {
      palette: [['Control', 'Shift', 'p']],
      rename: [['F2']],
    },
  );
});

test('of several bindings, the last with no when is shown, else the last', () => {
  assert.deepEqual(
    shortcuts([
      { key: 'ctrl+a', command: 'one' },
      { key: 'ctrl+b', command: 'one' },
      { key: 'ctrl+c', command: 'one', when: 'editorFocus' },
      { key: 'alt+a', command: 'two', when: 'a' },
      { key: 'alt+b', command: 'two', when: 'b' },
    ]),
    {
      one: [['Control', 'b']],
      two: [['Alt', 'b']],
    },
  );
});

test("the user's binding is after the defaults", () => {
  assert.deepEqual(
    shortcuts([{ key: 'ctrl+a', command: 'one' }], [{ key: 'ctrl+alt+a', command: 'one' }]),
    { one: [['Control', 'Alt', 'a']] },
  );
});

test("a removal takes away the defaults' bindings it names, and only theirs", () => {
  const defaults = [
    { key: 'ctrl+a', command: 'one' },
    { key: 'ctrl+b', command: 'one', when: 'editorFocus' },
    { key: 'ctrl+c', command: 'two' },
    { key: 'ctrl+d', command: 'two' },
    { key: 'ctrl+e', command: 'three', when: 'a' },
    { key: 'ctrl+f', command: 'three', when: 'b' },
  ];
  assert.deepEqual(
    shortcuts(defaults, [
      // By its keys: the binding with a when is what's left of it.
      { key: 'ctrl+a', command: '-one' },
      // By no keys: every binding of the command.
      { key: '', command: '-two' },
      // By its when, with the spaces written otherwise.
      { key: 'ctrl+f', command: '-three', when: '  b ' },
    ]),
    { one: [['Control', 'b']], three: [['Control', 'e']] },
  );
  // A removal leaves the user's own binding of the same keys.
  assert.deepEqual(
    shortcuts(
      [{ key: 'ctrl+a', command: 'one' }],
      [
        { key: 'ctrl+a', command: '-one' },
        { key: 'ctrl+a', command: 'one', when: 'editorFocus' },
      ],
    ),
    { one: [['Control', 'a']] },
  );
  // A removal whose when isn't the binding's leaves it.
  assert.deepEqual(
    shortcuts([{ key: 'ctrl+a', command: 'one' }], [{ key: 'ctrl+a', command: '-one', when: 'x' }]),
    { one: [['Control', 'a']] },
  );
});

test('a binding whose keys a later entry with no when gives to another command stops counting', () => {
  assert.deepEqual(
    shortcuts(
      [
        { key: 'ctrl+a', command: 'one' },
        { key: 'ctrl+b', command: 'one', when: 'x' },
        { key: 'ctrl+c', command: 'two' },
        { key: 'ctrl+d', command: 'three' },
        { key: 'ctrl+k ctrl+e', command: 'four' },
      ],
      [
        // Taken over by the user's file.
        { key: 'ctrl+a', command: 'other' },
        // An entry with a when takes nothing over.
        { key: 'ctrl+c', command: 'other', when: 'y' },
        // The same command's own binding takes nothing over.
        { key: 'ctrl+d', command: 'three', when: 'z' },
        // A chord's first press takes the chord over.
        { key: 'ctrl+k', command: 'other' },
      ],
    ),
    {
      one: [['Control', 'b']],
      two: [['Control', 'c']],
      three: [['Control', 'd']],
      other: [['Control', 'k']],
    },
  );
  // A command left with nothing has no label.
  assert.deepEqual(
    shortcuts([
      { key: 'ctrl+a', command: 'one' },
      { key: 'ctrl+a', command: 'two' },
    ]),
    { two: [['Control', 'a']] },
  );
});

test('a chord has no shortcut, even beside a single press with a when', () => {
  assert.deepEqual(
    shortcuts([
      { key: 'ctrl+k ctrl+o', command: 'openFolder' },
      { key: 'ctrl+o', command: 'open', when: 'x' },
      { key: 'ctrl+k ctrl+s', command: 'open' },
    ]),
    { openFolder: undefined, open: undefined },
  );
});

test('a key with no GDK name has no shortcut', () => {
  assert.equal(shortcutOf('ctrl+[KeyA]'), undefined);
  assert.equal(shortcutOf('ctrl+oem_102'), undefined);
  assert.equal(shortcutOf('ctrl+abnt_c1'), undefined);
  // Keys VS Code can't read aren't bindings at all.
  assert.deepEqual(shortcuts([{ key: 'ctrl+nosuchkey', command: 'one' }]), {});
});

test('the modifiers are Control, Alt, Shift and Super, in that order, before the GDK name', () => {
  assert.deepEqual(shortcutOf('meta+shift+alt+ctrl+pagedown'), [
    ['Control', 'Alt', 'Shift', 'Super', 'Page_Down'],
  ]);
  // As VS Code reads keys: in any case, with `-` between them, and win or cmd for meta.
  assert.deepEqual(shortcutOf('Ctrl-Shift-['), [['Control', 'Shift', 'bracketleft']]);
  assert.deepEqual(shortcutOf('win+oem_comma'), [['Super', 'comma']]);
  assert.deepEqual(shortcutOf('ctrl+='), [['Control', 'equal']]);
  assert.deepEqual(shortcutOf('ctrl+-'), [['Control', 'minus']]);
  assert.deepEqual(shortcutOf('shift+numpad_add'), [['Shift', 'KP_Add']]);
});

/**
 * Every name keybindings.json can give a key in VS Code 1.140: the user settings labels,
 * US and general, of each key code of `KeyCodeUtils` in `src/vs/base/common/keyCodes.ts` at
 * the 1.140.0 tag. The default keybindings write their keys with these, or as scan codes.
 */
// prettier-ignore
const VS_CODE_KEYS = [
  'unknown', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q',
  'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0',
  'enter', 'escape', 'backspace', 'tab', 'space', '-', 'oem_minus', '=', 'oem_plus', '[', 'oem_4',
  ']', 'oem_6', '\\', 'oem_5', ';', 'oem_1', "'", 'oem_7', '`', 'oem_3', ',', 'oem_comma', '.',
  'oem_period', '/', 'oem_2', 'capslock', 'f1', 'f2', 'f3', 'f4', 'f5', 'f6', 'f7', 'f8', 'f9',
  'f10', 'f11', 'f12', 'scrolllock', 'pausebreak', 'insert', 'home', 'pageup', 'delete', 'end',
  'pagedown', 'right', 'left', 'down', 'up', 'numlock', 'numpad_divide', 'numpad_multiply',
  'numpad_subtract', 'numpad_add', 'numpad1', 'numpad2', 'numpad3', 'numpad4', 'numpad5',
  'numpad6', 'numpad7', 'numpad8', 'numpad9', 'numpad0', 'numpad_decimal', 'oem_102',
  'contextmenu', 'f13', 'f14', 'f15', 'f16', 'f17', 'f18', 'f19', 'f20', 'f21', 'f22', 'f23',
  'f24', 'audiovolumemute', 'audiovolumeup', 'audiovolumedown', 'numpad_separator', 'abnt_c1',
  'clear', 'ctrl', 'shift', 'alt', 'meta', 'mediatracknext', 'mediatrackprevious', 'mediastop',
  'mediaplaypause', 'launchmediaplayer', 'launchmail', 'launchapp2', 'browsersearch',
  'browserhome', 'browserback', 'browserforward', 'keyincomposition', 'abnt_c2', 'oem_8',
];

test('every key VS Code 1.140 can bind has a GDK name or is knowingly left without one', () => {
  for (const key of VS_CODE_KEYS) {
    const shortcut = shortcutOf(`ctrl+${key}`);
    if (key in WITHOUT_GDK_KEY) {
      assert.equal(shortcut, undefined, key);
    } else {
      assert.ok(shortcut, `${key} has neither a GDK name nor a reason to have none`);
    }
  }
  for (const key of [...Object.keys(GDK_KEYS), ...Object.keys(WITHOUT_GDK_KEY)]) {
    assert.ok(VS_CODE_KEYS.includes(key), `${key} isn't a key VS Code can bind`);
  }
  for (const key of Object.keys(GDK_KEYS)) {
    assert.ok(!(key in WITHOUT_GDK_KEY), `${key} is in both lists`);
  }
  // A bare scan code is knowingly left without, whatever key it is.
  assert.equal(shortcutOf('ctrl+[Backquote]'), undefined);
});

test("every label of the snapshot that isn't a chord has a shortcut", () => {
  for (const [command, keybinding] of Object.entries(VS_CODE.table.keybindings)) {
    const label = keybinding.userSettingsLabel!;
    // Ctrl+NumPad0 among them, which Electron has no accelerator for.
    assert.equal(shortcutOf(label) !== undefined, !label.includes(' '), command);
  }
});

test("without the defaults, the snapshot's labels are used with the user's bindings over them", () => {
  const labels = fallbackLabelsOf(
    {
      kept: { label: 'Ctrl+A', userSettingsLabel: 'ctrl+a' },
      rebound: { label: 'Ctrl+B', userSettingsLabel: 'ctrl+b' },
      removed: { label: 'Ctrl+C', userSettingsLabel: 'ctrl+c' },
      removedAndRebound: { label: 'Ctrl+D', userSettingsLabel: 'ctrl+d' },
    },
    [
      { key: 'alt+b', command: 'rebound', when: 'x' },
      { key: 'alt+shift+b', command: 'rebound', when: 'y' },
      { key: 'ctrl+c', command: '-removed' },
      { key: 'ctrl+d', command: '-removedAndRebound' },
      { key: 'alt+d', command: 'removedAndRebound' },
      { key: 'alt+e', command: 'added' },
    ],
  );
  assert.deepEqual(Object.fromEntries(labels), {
    kept: 'ctrl+a',
    rebound: 'alt+shift+b',
    removedAndRebound: 'alt+d',
    added: 'alt+e',
  });
});

test('a keybindings file is read with its comments and trailing commas, and odd entries skipped', () => {
  const text = `// Place your key bindings in this file
[
  /* a block
     comment */
  { "key": "ctrl+/", "command": "comment", "when": "a // b" },
  { "key": "ctrl+a" },
  { "command": "-removal" },
  { "command": "no key" },
  "nonsense",
  { "key": "ctrl+\\"", "command": "quote", },
]
`;
  assert.deepEqual(bindingsOf(parseJsonc(text)), [
    { key: 'ctrl+/', command: 'comment', when: 'a // b' },
    { key: '', command: '-removal' },
    { key: 'ctrl+"', command: 'quote' },
  ]);
  assert.deepEqual(bindingsOf({ key: 'ctrl+a', command: 'one' }), []);
  assert.throws(() => parseJsonc('[ { "key": '));
});
