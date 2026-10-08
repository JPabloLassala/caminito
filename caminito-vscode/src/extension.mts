// The entry point the editor loads: it hands the real `vscode` module to the core.

import * as vscode from 'vscode';

import { start } from './global-menu.mts';
import type { GlobalMenu } from './global-menu.mts';

let running: GlobalMenu | undefined;

export function activate(): void {
  running = start(vscode, { platform: process.platform });
}

export function deactivate(): void {
  running?.dispose();
  running = undefined;
}
