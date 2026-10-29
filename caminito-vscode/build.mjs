// Bundles the extension into dist/extension.js, one CommonJS file with its dependencies in it
// and nothing native. `vscode` is the editor's own module. The target is the Node of the
// floor's Electron (1.68's Electron 17 has Node 16).
//
// The bundled dependencies are MIT, whose notice goes with every copy, so the build writes
// their notices to dist/THIRD-PARTY-NOTICES.txt beside the bundle (ADR 0051).
import { build } from 'esbuild';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const { metafile } = await build({
  entryPoints: ['src/extension.mts'],
  outfile: 'dist/extension.js',
  bundle: true,
  format: 'cjs',
  platform: 'node',
  target: 'node16',
  external: ['vscode'],
  minify: true,
  sourcemap: true,
  metafile: true,
  logLevel: 'warning',
});

writeFileSync('dist/THIRD-PARTY-NOTICES.txt', notices(bundledPackages(metafile)));

/** The directory of each package the bundle has a file of, in name order. */
function bundledPackages(metafile) {
  const directories = new Set();
  for (const input of Object.keys(metafile.inputs)) {
    const match = /^(.*node_modules\/(?:@[^/]+\/)?[^/]+)\//.exec(input);
    if (match) directories.add(match[1]);
  }
  return [...directories].sort();
}

/**
 * Each package's name, version and licence text. A package without a licence file gets the
 * MIT text with its package.json's author. A licence other than MIT fails the build: a new
 * one has to be checked against the extension's GPL-3.0-or-later first.
 */
function notices(directories) {
  const sections = directories.map((directory) => {
    const pkg = JSON.parse(readFileSync(join(directory, 'package.json'), 'utf8'));
    if (pkg.license !== 'MIT') {
      throw new Error(`${pkg.name} is ${pkg.license}, not MIT: check it before bundling it`);
    }
    const file = readdirSync(directory).find((name) => /^(licen[cs]e|copying)/i.test(name));
    const text = file
      ? readFileSync(join(directory, file), 'utf8').trim()
      : mitText(typeof pkg.author === 'string' ? pkg.author : pkg.author?.name);
    return `${pkg.name} ${pkg.version} (${pkg.license})\n\n${text}\n`;
  });
  return [
    'The global menu extension is GPL-3.0-or-later. dist/extension.js bundles these packages,',
    'each under its own licence.',
    '',
    ...sections.flatMap((section) => ['-'.repeat(80), '', section]),
  ].join('\n');
}

/** LICENSES/MIT.txt, with `author` in place of its copyright line's placeholder. */
function mitText(author) {
  if (!author) throw new Error('a package with no licence file names no author for its notice');
  return readFileSync('../LICENSES/MIT.txt', 'utf8')
    .replace('<year> <copyright holders>', author)
    .trim();
}
