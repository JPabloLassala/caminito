// The offer to hide the in-window menubar (ADR 0021), as the fake editor shows it (122).
//
// A claim that may bring the offer is sent once the offer is made or passed over, so a
// claim the stand-in has had says the offer that came with it is settled.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { AGENTS_WORKSPACE, USER_DIR, fixture } from './fixture.mts';
import type { FakeEditor } from './harness.mts';

const OFFER = 'caminito.globalMenu.offerToHideMenuBar';
const VISIBILITY = 'window.menuBarVisibility';
const TEXT =
  "The panel now shows this window's menus. Hide the menu bar in the window? Pressing Alt shows it again.";

/** Lets an answer to a notification be acted on. */
const settle = () => new Promise((resolve) => setImmediate(resolve));

/** What was written to `window.menuBarVisibility`. */
const visibilityWritten = (editor: FakeEditor) =>
  editor.written.filter(([setting]) => setting === VISIBILITY);

for (const visibility of ['classic', 'visible']) {
  test(`a claim with the menubar ${visibility} offers once and turns the offer off`, async (t) => {
    const f = await fixture(t, { settings: { [VISIBILITY]: visibility, [OFFER]: true } });
    await f.claimed(1);
    await f.editor.notified(1);
    const [shown] = f.editor.notifications;
    assert.equal(shown!.message, TEXT);
    assert.deepEqual(shown!.items, ['Hide Menu Bar', 'Keep Menu Bar']);
    assert.deepEqual(f.editor.written, [[OFFER, false, true]]);
    f.editor.focus(false);
    f.editor.focus(true);
    await f.claimed(2);
    await f.editor.notified(1);
  });
}

for (const visibility of ['toggle', 'hidden', 'compact']) {
  test(`a claim with the menubar ${visibility} offers nothing`, async (t) => {
    const f = await fixture(t, { settings: { [VISIBILITY]: visibility, [OFFER]: true } });
    await f.claimed(1);
    await f.editor.notified(0);
    assert.deepEqual(f.editor.written, []);
  });
}

test('a claim with the offer turned off offers nothing', async (t) => {
  const f = await fixture(t, { settings: { [VISIBILITY]: 'classic', [OFFER]: false } });
  await f.claimed(1);
  await f.editor.notified(0);
  assert.deepEqual(f.editor.written, []);
});

test('Hide Menu Bar sets the menubar to toggle in the user settings', async (t) => {
  const f = await fixture(t, { settings: { [VISIBILITY]: 'classic', [OFFER]: true } });
  await f.editor.notified(1);
  f.editor.notifications[0]!.answer('Hide Menu Bar');
  await settle();
  assert.deepEqual(visibilityWritten(f.editor), [[VISIBILITY, 'toggle', true]]);
});

for (const [what, answer] of [
  ['Keep Menu Bar', 'Keep Menu Bar'],
  ['a notification closed with no answer', undefined],
] as const) {
  test(`${what} writes nothing to the menubar's setting`, async (t) => {
    const f = await fixture(t, { settings: { [VISIBILITY]: 'visible', [OFFER]: true } });
    await f.editor.notified(1);
    f.editor.notifications[0]!.answer(answer);
    await settle();
    assert.deepEqual(visibilityWritten(f.editor), []);
    assert.equal(f.editor.settings[VISIBILITY], 'visible');
  });
}

test('two windows of one editor claiming together make one offer', async (t) => {
  const settings = { [VISIBILITY]: 'classic', [OFFER]: true };
  const f = await fixture(t, { settings, focused: false });
  const other = await f.another({ settings, focused: false });
  f.editor.focus(true);
  other.editor.focus(true);
  await f.claimed(2);
  await settle();
  assert.equal(f.editor.notifications.length + other.editor.notifications.length, 1);
});

test('a window of another editor offers too', async (t) => {
  const settings = { [VISIBILITY]: 'classic', [OFFER]: true };
  const f = await fixture(t, { settings, focused: false });
  const other = await f.another({ settings, focused: false, parentPid: 4343 });
  f.editor.focus(true);
  other.editor.focus(true);
  await f.claimed(2);
  await f.editor.notified(1);
  await other.editor.notified(1);
});

test('the agents window claims and offers nothing', async (t) => {
  const f = await fixture(t, {
    settings: { [VISIBILITY]: 'classic', [OFFER]: true },
    workspaceFile: AGENTS_WORKSPACE,
    userDir: USER_DIR,
  });
  await f.claimed(1);
  await f.editor.notified(0);
  assert.deepEqual(f.editor.written, []);
});

test('a workspace file of that name elsewhere is no agents window', async (t) => {
  const f = await fixture(t, {
    settings: { [VISIBILITY]: 'classic', [OFFER]: true },
    workspaceFile: '/home/user/projects/agent-sessions.code-workspace',
    userDir: USER_DIR,
  });
  await f.claimed(1);
  await f.editor.notified(1);
});

test('the offer turned on again comes at the next claim, from the window with the name', async (t) => {
  const settings = { [VISIBILITY]: 'classic', [OFFER]: true };
  const f = await fixture(t, { settings });
  await f.editor.notified(1);
  const other = await f.another({ settings: { ...settings, [OFFER]: false } });
  await f.claimed(2);
  f.editor.set({ [OFFER]: true });
  other.editor.set({ [OFFER]: true });
  other.editor.focus(false);
  other.editor.focus(true);
  await f.claimed(3);
  await other.editor.notified(0);
  f.editor.focus(false);
  f.editor.focus(true);
  await f.claimed(4);
  await f.editor.notified(2);
  assert.equal(f.editor.settings[OFFER], false);
});
