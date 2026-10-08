// An editor older than the floor, and a failed start (ADR 0022): what's there is used, one
// line says so, and nothing throws into the extension host (116).

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { FLOOR, start } from '../src/global-menu.mts';
import { fixture } from './fixture.mts';
import { Caminito, FakeEditor, PrivateBus, until } from './harness.mts';

test("the floor is the manifest's engines.vscode", () => {
  const manifest = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
  assert.equal(manifest.engines.vscode, `^${FLOOR}`);
});

test('an editor without the tab-groups and notebook APIs is served and claimed, with one line logged', async (t) => {
  const f = await fixture(t, { version: '1.66.2', lacking: { tabGroups: true, notebooks: true } });
  await f.claimed(1);
  assert.ok(await f.menu());
  f.editor.textEditor.fire(undefined);
  await f.claimed(2);
  f.editor.focus(false);
  f.editor.focus(true);
  await f.claimed(3);
  assert.deepEqual(f.editor.logged, [
    "the editor is 1.66.2 and the extension's floor is 1.68.0, so no claim on a change of tab groups or a change of the notebook editor",
  ]);
});

test('an editor older than the floor with every API it uses logs both versions', async (t) => {
  const f = await fixture(t, { version: '1.67.0' });
  await f.claimed(1);
  assert.deepEqual(f.editor.logged, ["the editor is 1.67.0 and the extension's floor is 1.68.0"]);
});

test("a session bus that can't be reached is logged, and the start settles", async (t) => {
  const f = await fixture(t, { busAddress: 'unix:path=/nonexistent/caminito-test-bus' });
  assert.equal(f.editor.logged.length, 1);
  assert.match(f.editor.logged[0]!, /^can't start: connect ENOENT/);
});

test('an API that throws at activation is logged, and nothing is left on the bus', async (t) => {
  const bus = await PrivateBus.start();
  t.after(() => bus.close());
  const caminitoBus = await bus.connect();
  const caminito = new Caminito(caminitoBus, true);
  await caminito.own();
  const editor = new FakeEditor(true);
  const api = editor.api();
  const broken = {
    ...api,
    window: {
      ...api.window,
      createOutputChannel: api.window.createOutputChannel,
      onDidChangeWindowState: () => {
        throw new Error('no window events');
      },
    },
  };
  const extension = start(broken, { platform: 'linux', busAddress: bus.address });
  t.after(() => extension.dispose());
  await extension.ready;
  assert.deepEqual(editor.logged, ["can't start: no window events"]);
  await until(
    async () => (await bus.othersThan(caminitoBus)).length === 0,
    'the extension left the bus',
  );
});
