// The developer command that makes the menu table (ADR 0019): it starts VS Code under Xvfb
// with a native title bar and a throwaway profile, once with a folder open and once with a
// multi-root workspace open, waits each time until `lastKnownMenubarData` in its
// `storage.json` stops changing, and writes the merged table to menu/table.json. It fails on
// an annotation whose command the table lacks.
//
//   npm run snapshot -- [--editor <dir>] [--out <file>] [--no-xvfb] [--settle <ms>]
//
// `--editor` is the directory of Microsoft's build, /usr/share/code by default, as
// visual-studio-code-bin installs it: the Electron binary `code` and resources/app. The
// table records its version and commit. `--no-xvfb` runs it on the display there is.

import { spawn } from 'node:child_process';
import { mkdtemp, readFile, readdir, rm, writeFile, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { parseArgs } from 'node:util';

import type { Annotations } from '../src/menu-table.mts';
import { staleAnnotations, tableOf } from './menubar.mts';
import type { MenubarData } from './menubar.mts';

const here = dirname(new URL(import.meta.url).pathname);
const { values: options } = parseArgs({
  options: {
    editor: { type: 'string', default: '/usr/share/code' },
    out: { type: 'string', default: join(here, '../menu/table.json') },
    annotations: { type: 'string', default: join(here, '../menu/annotations.json') },
    'no-xvfb': { type: 'boolean', default: false },
    settle: { type: 'string', default: '3000' },
    timeout: { type: 'string', default: '90000' },
  },
});
const editor = resolve(options.editor);
const settle = Number(options.settle);
const timeout = Number(options.timeout);

const app = join(editor, 'resources/app');
const made = {
  version: (JSON.parse(await readFile(join(app, 'package.json'), 'utf8')) as { version: string })
    .version,
  commit:
    (JSON.parse(await readFile(join(app, 'product.json'), 'utf8')) as { commit?: string }).commit ??
    '',
};
const annotations = JSON.parse(await readFile(options.annotations, 'utf8')) as Annotations;

const snapshots: MenubarData[] = [];
for (const state of ['folder', 'workspace'] as const) {
  console.error(`snapshot: VS Code ${made.version} with a ${state} open`);
  snapshots.push(await snapshot(state));
}
const table = tableOf(made, snapshots);
const stale = staleAnnotations(table, annotations);
if (stale.length > 0) {
  console.error(`snapshot: the table has no item of these annotated commands: ${stale.join(', ')}`);
  process.exit(1);
}
await writeFile(options.out, `${JSON.stringify(table, null, 2)}\n`);
console.error(`snapshot: wrote ${options.out}`);

/** One run of the editor, with a folder or a two-folder workspace open. */
async function snapshot(state: 'folder' | 'workspace'): Promise<MenubarData> {
  const work = await mkdtemp(join(tmpdir(), 'caminito-snapshot-'));
  try {
    const profile = join(work, 'user-data');
    await mkdir(join(profile, 'User'), { recursive: true });
    await writeFile(
      join(profile, 'User/settings.json'),
      JSON.stringify({
        'window.titleBarStyle': 'native',
        'workbench.startupEditor': 'none',
        'update.mode': 'none',
        'telemetry.telemetryLevel': 'off',
      }),
    );
    for (const folder of ['a', 'b']) {
      await mkdir(join(work, folder));
    }
    let target = join(work, 'a');
    if (state === 'workspace') {
      target = join(work, 'ab.code-workspace');
      await writeFile(target, JSON.stringify({ folders: [{ path: 'a' }, { path: 'b' }] }));
    }
    const args = [
      // Electron's sandbox needs a setuid helper or user namespaces, and isn't what's measured.
      '--no-sandbox',
      '--disable-gpu',
      `--user-data-dir=${profile}`,
      `--extensions-dir=${join(work, 'extensions')}`,
      '--skip-welcome',
      '--skip-release-notes',
      '--disable-workspace-trust',
      target,
    ];
    const code = join(editor, 'code');
    const [command, ...rest] = options['no-xvfb']
      ? [code, ...args]
      : ['xvfb-run', '--auto-servernum', code, ...args];
    // Its own process group, so that the editor's helpers end with it.
    const child = spawn(command!, rest, { detached: true, stdio: 'ignore' });
    const exited = new Promise<void>((resolve) => child.once('exit', () => resolve()));
    try {
      return await settled(profile, child);
    } finally {
      try {
        process.kill(-child.pid!, 'SIGTERM');
      } catch {
        // It's gone already.
      }
      await exited;
    }
  } finally {
    await rm(work, { recursive: true, force: true });
  }
}

/** `lastKnownMenubarData` once it has stayed the same for `settle` ms. */
async function settled(profile: string, child: { exitCode: number | null }): Promise<MenubarData> {
  const deadline = Date.now() + timeout;
  let last: string | undefined;
  let since = Date.now();
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(`the editor exited with ${child.exitCode} before the menubar settled`);
    }
    const now = await menubarData(profile);
    if (now !== last) {
      [last, since] = [now, Date.now()];
    } else if (now !== undefined && Date.now() - since >= settle) {
      return JSON.parse(now) as MenubarData;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`lastKnownMenubarData didn't settle within ${timeout} ms`);
}

/** `lastKnownMenubarData` as written, from any `storage.json` under the profile. */
async function menubarData(profile: string): Promise<string | undefined> {
  const files = (await readdir(profile, { recursive: true }))
    .filter((file) => basename(file) === 'storage.json')
    .sort();
  for (const file of files) {
    try {
      const storage = JSON.parse(await readFile(join(profile, file), 'utf8')) as Record<
        string,
        unknown
      >;
      if (storage['lastKnownMenubarData'] !== undefined) {
        return JSON.stringify(storage['lastKnownMenubarData']);
      }
    } catch {
      // Half written: the next look reads it whole.
    }
  }
  return undefined;
}
