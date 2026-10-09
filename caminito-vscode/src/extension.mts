// The entry point the editor loads: it hands the real `vscode` module to the core.

import { dirname } from 'node:path';

import * as vscode from 'vscode';

import { start } from './global-menu.mts';
import type { GlobalMenu } from './global-menu.mts';

let running: GlobalMenu | undefined;

export function activate(context: vscode.ExtensionContext): void {
  // The extension's global storage is the default profile's whatever the window's, at
  // `<user data>/User/globalStorage/caminito.global-menu`, as ticket 02 found in 1.140.
  const userDir = dirname(dirname(context.globalStorageUri.fsPath));
  running = start(vscode, { platform: process.platform, parentPid: process.ppid, userDir });
}

export function deactivate(): void {
  running?.dispose();
  running = undefined;
}
