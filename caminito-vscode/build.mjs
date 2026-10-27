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
      : mit(typeof pkg.author === 'string' ? pkg.author : pkg.author?.name);
    return `${pkg.name} ${pkg.version} (${pkg.license})\n\n${text}\n`;
  });
  return [
    'The global menu extension is GPL-3.0-or-later. dist/extension.js bundles these packages,',
    'each under its own licence.',
    '',
    ...sections.flatMap((section) => ['-'.repeat(80), '', section]),
  ].join('\n');
}

function mit(author) {
  return `MIT License

Copyright (c) ${author}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.`;
}
