// The gate (ADR 0018): the menu is served and claimed only on Linux, while org.mate.Caminito
// has an owner and GlobalMenuOffered is true, and it follows both (116).

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { fixture } from './fixture.mts';

test('off Linux the extension never joins the bus, and logs nothing', async (t) => {
  const f = await fixture(t, { platform: 'darwin' });
  f.editor.focus(false);
  f.editor.focus(true);
  f.editor.textEditor.fire(undefined);
  assert.deepEqual(await f.names(), []);
  assert.deepEqual(f.caminito.claims, []);
  assert.deepEqual(f.editor.logged, []);
});

test('with no owner of org.mate.Caminito nothing is served, until it has one', async (t) => {
  const f = await fixture(t, { owned: false });
  assert.equal((await f.names()).length, 1, 'the extension watches from the bus');
  assert.equal(await f.menu(), undefined);
  await f.caminito.own();
  await f.served(true);
  await f.claimed(1);
});

test('with GlobalMenuOffered false nothing is served or claimed, until it turns true', async (t) => {
  const f = await fixture(t, { offered: false });
  f.editor.focus(false);
  f.editor.focus(true);
  f.editor.textEditor.fire(undefined);
  assert.equal(await f.menu(), undefined);
  f.caminito.offer(true);
  await f.served(true);
  await f.claimed(1);
});

test('the menu stops being served when GlobalMenuOffered turns false, and comes back', async (t) => {
  const f = await fixture(t);
  await f.claimed(1);
  f.caminito.offer(false);
  await f.served(false);
  f.editor.focus(false);
  f.editor.focus(true);
  f.caminito.offer(true);
  await f.served(true);
  await f.claimed(2);
});

test('the menu stops being served when Caminito leaves the bus, and comes back with it', async (t) => {
  const f = await fixture(t);
  await f.claimed(1);
  await f.caminito.leave();
  await f.served(false);
  await f.caminito.own();
  await f.served(true);
  await f.claimed(2);
});

test('a window that runs normally logs nothing', async (t) => {
  const f = await fixture(t);
  await f.claimed(1);
  assert.deepEqual(f.editor.logged, []);
});
