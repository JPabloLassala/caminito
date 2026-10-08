// Bundles the extension into dist/extension.js, one CommonJS file with its dependencies in it
// and nothing native. `vscode` is the editor's own module. The target is the Node of the
// floor's Electron (1.68's Electron 17 has Node 16).
import { build } from 'esbuild';

await build({
  entryPoints: ['src/extension.mts'],
  outfile: 'dist/extension.js',
  bundle: true,
  format: 'cjs',
  platform: 'node',
  target: 'node16',
  external: ['vscode'],
  minify: true,
  sourcemap: true,
  logLevel: 'warning',
});
