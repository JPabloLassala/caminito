// The part of the `vscode` API the global menu extension uses, which is the seam its tests
// hand a fake to (116).
//
// The floor in package.json is the oldest release with all of it. A built-in extension is
// loaded below its floor all the same (ADR 0022), so what came late is optional here, and
// the extension looks for it before using it.

import type { Event, Uri, WindowState } from 'vscode';

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
    readonly tabGroups?: {
      readonly onDidChangeTabGroups: Event<unknown>;
      readonly onDidChangeTabs: Event<unknown>;
    };
    createOutputChannel(name: string): { appendLine(line: string): void; dispose(): void };
    /** Settles on the item chosen, or undefined when the notification is closed with none. */
    showInformationMessage(message: string, ...items: string[]): PromiseLike<string | undefined>;
  };
  readonly workspace: {
    /** Undefined when no folder is open. */
    readonly workspaceFolders: readonly { readonly uri: { toString(): string } }[] | undefined;
    /** Defined when a multi-root workspace is open, an untitled one among them. */
    readonly workspaceFile:
      | { readonly scheme: string; readonly fsPath: string; toString(): string }
      | undefined;
    readonly onDidChangeWorkspaceFolders: Event<unknown>;
    getConfiguration(): {
      get(section: string): unknown;
      /** `global` true writes the user's settings. */
      update(section: string, value: unknown, global: boolean): PromiseLike<void>;
    };
    readonly onDidChangeConfiguration: Event<{ affectsConfiguration(section: string): boolean }>;
    /** Reads the default keybindings, at `vscode://defaultsettings/keybindings.json`. */
    readonly fs: { readFile(uri: Uri): PromiseLike<Uint8Array> };
  };
  readonly Uri: { parse(value: string): Uri };
  readonly extensions: { readonly onDidChange: Event<void> };
  readonly env: {
    /** The editor's own directory, where its `product.json` is. */
    readonly appRoot: string;
  };
  readonly commands: {
    getCommands(filterInternal?: boolean): PromiseLike<string[]>;
    executeCommand(command: string, ...args: unknown[]): PromiseLike<unknown>;
  };
}
