// caminito-vscode-git's link script (117, ADR 0022), run against the five built-in directories
// under a temporary root: it links only where a directory exists, running it twice changes
// nothing, and it removes only its own link.

import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { lstat, mkdir, mkdtemp, readlink, rm, symlink } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const SCRIPT = fileURLToPath(
  new URL('../../packaging/arch/caminito-vscode-git/caminito-vscode-link', import.meta.url),
);
const EXTENSION = '/usr/share/caminito-vscode/global-menu';
const NAME = 'caminito.global-menu';

const CODE = '/usr/share/code/resources/app/extensions';
const OSS = '/usr/lib/code/extensions';
const VSCODIUM_BIN = '/opt/vscodium-bin/resources/app/extensions';
const VSCODIUM = '/usr/share/vscodium/resources/app/extensions';
const CURSOR = '/usr/share/cursor/resources/app/extensions';
const FIVE = [CODE, OSS, VSCODIUM_BIN, VSCODIUM, CURSOR];

/** Runs the script under `root`, with `stdin` as the hook's targets. */
function run(root: string, args: string[], stdin = ''): Promise<{ stdout: string; stderr: string }> {
  return new Promise((resolve, reject) => {
    const child = execFile(
      'sh',
      [SCRIPT, ...args],
      { env: { ...process.env, CAMINITO_VSCODE_ROOT: root } },
      (error, stdout, stderr) => (error ? reject(error) : resolve({ stdout, stderr })),
    );
    child.stdin!.end(stdin);
  });
}

async function withRoot(body: (root: string) => Promise<void>): Promise<void> {
  const root = await mkdtemp(join(tmpdir(), 'caminito-vscode-link-'));
  try {
    await body(root);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

async function editors(root: string, directories: string[]): Promise<void> {
  for (const directory of directories) {
    await mkdir(join(root, directory), { recursive: true });
  }
}

/** What is at the link's path in `directory`: our link, another link, a directory, or nothing. */
async function at(root: string, directory: string): Promise<string | undefined> {
  const path = join(root, directory, NAME);
  const stat = await lstat(path).catch(() => undefined);
  if (stat === undefined) {
    return undefined;
  }
  return stat.isSymbolicLink() ? await readlink(path) : 'directory';
}

async function everywhere(root: string): Promise<(string | undefined)[]> {
  return Promise.all(FIVE.map((directory) => at(root, directory)));
}

test('it lists the five built-in directories', async () => {
  const { stdout } = await run('', ['directories']);
  assert.deepEqual(stdout.trim().split('\n'), FIVE);
});

test('it links only where a directory exists', async () => {
  await withRoot(async (root) => {
    await editors(root, [CODE, VSCODIUM]);
    await run(root, ['link']);
    assert.deepEqual(await everywhere(root), [EXTENSION, undefined, undefined, EXTENSION, undefined]);
    await lstat(join(root, '/usr/lib/code')).then(
      () => assert.fail('it made a directory of an editor that is not installed'),
      () => undefined,
    );
  });
});

test('linking twice changes nothing', async () => {
  await withRoot(async (root) => {
    await editors(root, FIVE);
    await run(root, ['link']);
    const { stderr } = await run(root, ['link']);
    assert.equal(stderr, '');
    assert.deepEqual(await everywhere(root), FIVE.map(() => EXTENSION));
  });
});

test('unlinking removes every link and no directory', async () => {
  await withRoot(async (root) => {
    await editors(root, [OSS, CURSOR]);
    await run(root, ['link']);
    await run(root, ['unlink']);
    await run(root, ['unlink']);
    assert.deepEqual(await everywhere(root), FIVE.map(() => undefined));
    await lstat(join(root, OSS));
    await lstat(join(root, CURSOR));
  });
});

test('it leaves alone what is not its own link', async () => {
  await withRoot(async (root) => {
    await editors(root, [CODE, OSS, join(VSCODIUM, NAME)]);
    await symlink('/home/someone/global-menu', join(root, CODE, NAME));
    await run(root, ['link']);
    await run(root, ['unlink']);
    assert.deepEqual(await everywhere(root), [
      '/home/someone/global-menu',
      undefined,
      undefined,
      'directory',
      undefined,
    ]);
  });
});

test("the removal hook unlinks only the directories of the editor it's removing", async () => {
  await withRoot(async (root) => {
    await editors(root, [CODE, VSCODIUM_BIN]);
    await run(root, ['link']);
    // pacman's targets have no leading slash, and a directory ends in one.
    const targets = [
      'usr/share/code/resources/app/extensions/',
      'usr/share/code/resources/app/extensions/git/package.json',
    ];
    await run(root, ['unlink', '--targets'], targets.join('\n') + '\n');
    assert.deepEqual(await everywhere(root), [undefined, undefined, EXTENSION, undefined, undefined]);
  });
});
