// When the extension claims its menu (ADR 0018), as the stand-in for Caminito sees it (116).
//
// A claim that shouldn't have been made would reach the stand-in before a later one from
// the same connection, so a test that expects none makes a claim afterwards and counts.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { MENU_PATH } from '../src/global-menu.mts';
import { fixture } from './fixture.mts';

test('a window focused at activation claims its menu, under the name that serves it', async (t) => {
  const f = await fixture(t);
  await f.claimed(1);
  assert.deepEqual(f.caminito.claims, [{ sender: (await f.names())[0], path: MENU_PATH }]);
});

test('a window unfocused at activation claims nothing until it gains the focus', async (t) => {
  const f = await fixture(t, { focused: false });
  f.editor.focus(true);
  await f.claimed(1);
});

test('focus gained claims each time, and focus lost never does', async (t) => {
  const f = await fixture(t);
  await f.claimed(1);
  f.editor.focus(false);
  f.editor.focus(true);
  await f.claimed(2);
  f.editor.focus(false);
  f.editor.focus(true);
  await f.claimed(3);
});

test('a change of editor or of tab groups claims while the window is focused', async (t) => {
  const f = await fixture(t);
  await f.claimed(1);
  f.editor.textEditor.fire(undefined);
  await f.claimed(2);
  f.editor.notebookEditor.fire(undefined);
  await f.claimed(3);
  f.editor.tabGroups.fire(undefined);
  await f.claimed(4);
});

test('a change of editor or of tab groups claims nothing while the window is unfocused', async (t) => {
  const f = await fixture(t, { focused: false });
  f.editor.textEditor.fire(undefined);
  f.editor.notebookEditor.fire(undefined);
  f.editor.tabGroups.fire(undefined);
  f.editor.focus(true);
  await f.claimed(1);
});

test('GlobalMenuOffered turning true claims for a focused window', async (t) => {
  const f = await fixture(t, { offered: false });
  f.editor.focus(false);
  f.editor.focus(true);
  f.editor.textEditor.fire(undefined);
  f.caminito.offer(true);
  await f.claimed(1);
});

test('GlobalMenuOffered turning true claims nothing for an unfocused window', async (t) => {
  const f = await fixture(t, { offered: false, focused: false });
  f.caminito.offer(true);
  // The offer's claim would come before the one on focus.
  await f.served(true);
  f.editor.focus(true);
  await f.claimed(1);
});

test('org.mate.Caminito gaining an owner claims for a focused window', async (t) => {
  const f = await fixture(t, { owned: false });
  await f.caminito.own();
  await f.claimed(1);
});

test('org.mate.Caminito gaining an owner claims nothing for an unfocused window', async (t) => {
  const f = await fixture(t, { owned: false, focused: false });
  await f.caminito.own();
  await f.served(true);
  f.editor.focus(true);
  await f.claimed(1);
});

test('a restarted Caminito is claimed from again', async (t) => {
  const f = await fixture(t);
  await f.claimed(1);
  await f.caminito.leave();
  await f.caminito.own();
  await f.claimed(2);
});
