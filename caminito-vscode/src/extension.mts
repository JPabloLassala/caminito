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
    // The default profile's global storage, `<user data>/User/globalStorage/<extension>`,
    // whatever the window's profile is.
    settingsDirectory: dirname(dirname(context.globalStorageUri.fsPath)),
  });
}

export function deactivate(): void {
  running?.dispose();
  running = undefined;
}
