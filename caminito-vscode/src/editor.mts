// The part of the `vscode` API the global menu extension uses, which is the seam its tests
// hand a fake to (116).
//
// The floor in package.json is the oldest release with all of it. A built-in extension is
// loaded below its floor all the same (ADR 0022), so what came late is optional here, and
// the extension looks for it before using it.

import type { Event, WindowState } from 'vscode';

/** Something to undo, as the editor's `Disposable`. */
export interface Disposable {
  dispose(): unknown;
}

/** The `vscode` module, as much of it as the extension uses. */
export interface Editor {
  /** The editor's version, as `vscode.version`. */
  readonly version: string;
  readonly window: {
    readonly state: WindowState;
    readonly onDidChangeWindowState: Event<WindowState>;
    readonly onDidChangeActiveTextEditor: Event<unknown>;
    /** 1.68. */
    readonly onDidChangeActiveNotebookEditor?: Event<unknown>;
    /** 1.67. */
    readonly tabGroups?: { readonly onDidChangeTabGroups: Event<unknown> };
    createOutputChannel(name: string): { appendLine(line: string): void; dispose(): void };
  };
  readonly commands: {
    executeCommand(command: string): PromiseLike<unknown>;
  };
}
