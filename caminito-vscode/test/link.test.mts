// The link script both distributions' packages run (117, 193, ADR 0022), against the six
// built-in directories under a temporary root: it links only where an editor is installed,
// running it twice changes nothing, it removes only its own link, and a link an editor's
// removal left behind goes on the next run.

import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { lstat, mkdir, mkdtemp, readlink, rm, symlink } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const SCRIPT = fileURLToPath(
  new URL('../link/caminito-vscode-link', import.meta.url),
);
const EXTENSION = '/usr/share/caminito-vscode/global-menu';
const NAME = 'caminito.global-menu';

const CODE = '/usr/share/code/resources/app/extensions';
const OSS = '/usr/lib/code/extensions';
const VSCODIUM_BIN = '/opt/vscodium-bin/resources/app/extensions';
const VSCODIUM = '/usr/share/vscodium/resources/app/extensions';
const CURSOR = '/usr/share/cursor/resources/app/extensions';
// VSCodium's own .deb.
const CODIUM = '/usr/share/codium/resources/app/extensions';
const BUILT_INS = [CODE, OSS, VSCODIUM_BIN, VSCODIUM, CURSOR, CODIUM];

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

/** Installs an editor in each of `directories`: the directory, with a built-in in it. */
async function editors(root: string, directories: string[]): Promise<void> {
  for (const directory of directories) {
    await mkdir(join(root, directory, 'git'), { recursive: true });
  }
}

/** Removes the editor's own built-ins from `directory`, as its package manager does. */
async function uninstall(root: string, directory: string): Promise<void> {
  await rm(join(root, directory, 'git'), { recursive: true });
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
  return Promise.all(BUILT_INS.map((directory) => at(root, directory)));
}

test('it lists the six built-in directories', async () => {
  const { stdout } = await run('', ['directories']);
  assert.deepEqual(stdout.trim().split('\n'), BUILT_INS);
});

test('it links only where an editor is installed', async () => {
  await withRoot(async (root) => {
    await editors(root, [CODE, VSCODIUM]);
    await run(root, ['link']);
    assert.deepEqual(await everywhere(root), [
      EXTENSION,
      undefined,
      undefined,
      EXTENSION,
      undefined,
      undefined,
    ]);
    await lstat(join(root, '/usr/lib/code')).then(
      () => assert.fail('it made a directory of an editor that is not installed'),
      () => undefined,
    );
  });
});

test('linking twice changes nothing', async () => {
  await withRoot(async (root) => {
    await editors(root, BUILT_INS);
    await run(root, ['link']);
    const { stderr } = await run(root, ['link']);
    assert.equal(stderr, '');
    assert.deepEqual(await everywhere(root), BUILT_INS.map(() => EXTENSION));
  });
});

test('unlinking removes every link and no directory', async () => {
  await withRoot(async (root) => {
    await editors(root, [OSS, CURSOR]);
    await run(root, ['link']);
    await run(root, ['unlink']);
    await run(root, ['unlink']);
    assert.deepEqual(await everywhere(root), BUILT_INS.map(() => undefined));
    await lstat(join(root, OSS));
    await lstat(join(root, CURSOR));
  });
});

test('it leaves alone what is not its own link', async () => {
  await withRoot(async (root) => {
    await editors(root, [CODE, OSS, VSCODIUM]);
    await mkdir(join(root, VSCODIUM, NAME));
    await symlink('/home/someone/global-menu', join(root, CODE, NAME));
    await run(root, ['link']);
    await run(root, ['unlink']);
    assert.deepEqual(await everywhere(root), [
      '/home/someone/global-menu',
      undefined,
      undefined,
      'directory',
      undefined,
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
    assert.deepEqual(await everywhere(root), [
      undefined,
      undefined,
      EXTENSION,
      undefined,
      undefined,
      undefined,
    ]);
  });
});

test("VSCodium's .deb path is linked", async () => {
  await withRoot(async (root) => {
    await editors(root, [CODIUM]);
    await run(root, ['link']);
    assert.equal(await at(root, CODIUM), EXTENSION);
  });
});

// Debian's triggers run the script with no targets, for an editor's removal as for its install
// (204). The removal leaves the directory holding the link alone, which dpkg can't remove.
test('linking with no targets drops a link whose editor is gone, and keeps the rest', async () => {
  await withRoot(async (root) => {
    await editors(root, [CODE, CODIUM]);
    await run(root, ['link']);
    await uninstall(root, CODIUM);
    const { stderr } = await run(root, ['link']);
    assert.equal(stderr, '');
    assert.equal(await at(root, CODE), EXTENSION);
    assert.equal(await at(root, CODIUM), undefined);
    await lstat(join(root, CODIUM));
  });
});

test("an editor's directory left empty is not linked", async () => {
  await withRoot(async (root) => {
    await mkdir(join(root, CURSOR), { recursive: true });
    await run(root, ['link']);
    assert.equal(await at(root, CURSOR), undefined);
  });
});

test("a link that isn't its own stays where the editor is gone", async () => {
  await withRoot(async (root) => {
    await mkdir(join(root, CODE), { recursive: true });
    await symlink('/home/someone/global-menu', join(root, CODE, NAME));
    await run(root, ['link']);
    assert.equal(await at(root, CODE), '/home/someone/global-menu');
  });
});
