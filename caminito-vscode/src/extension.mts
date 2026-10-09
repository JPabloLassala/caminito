// The entry point the editor loads: it hands the real `vscode` module to the core.

import { dirname } from 'node:path';

import * as vscode from 'vscode';

import { start } from './global-menu.mts';
import type { GlobalMenu } from './global-menu.mts';

let running: GlobalMenu | undefined;

export function activate(context: vscode.ExtensionContext): void {
  running = start(vscode, {
    platform: process.platform,
    parentPid: process.ppid,
    // `<user data>/User/globalStorage/<extension>`, the default profile's whatever the
    // window's profile is, as ticket 12 read VS Code's source. It hasn't been run (122).
    settingsDirectory: dirname(dirname(context.globalStorageUri.fsPath)),
  });
}

export function deactivate(): void {
  running?.dispose();
  running = undefined;
}
