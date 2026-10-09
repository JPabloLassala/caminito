// Open Recent's entries (ADR 0020): VS Code's recently opened list, read with the internal
// `_workbench.getRecentlyOpened`, laid out and labelled as its own menubar has them.

import type { MenuNode } from './dbusmenu.mts';

/** The internal command that answers the recently opened list. */
export const GET_RECENTLY_OPENED = '_workbench.getRecentlyOpened';

/** The table's item the entries are put before: More…. */
export const MORE = 'workbench.action.openRecent';

/** The commands that open an entry or clear the list, after whose return the list is read again. */
export const CHANGES_THE_LIST: readonly string[] = [
  'vscode.openFolder',
  '_files.windowOpen',
  'vscode.open',
  'workbench.action.clearRecentFiles',
];

/** How many folders and workspaces, and how many files, the menu shows. */
export const MAX_ENTRIES = 10;

/** The longest label, cut in the middle beyond it. */
export const MAX_LABEL = 120;

const OPEN_FOLDER = 'vscode.openFolder';
const WINDOW_OPEN = '_files.windowOpen';
const OPEN = 'vscode.open';
const WORKSPACE_EXTENSION = '.code-workspace';

/** A URI, as the editor hands one over: the parts of it a label is made of. */
export interface Uri {
  readonly scheme: string;
  readonly authority: string;
  readonly path: string;
}

interface Recent {
  /** The label VS Code was given for the entry, which it shows as it is. */
  readonly label?: string;
  /** The remote the entry is on, as `ssh-remote+host`. */
  readonly remoteAuthority?: string;
}

export interface RecentFolder extends Recent {
  readonly folderUri: Uri;
}

export interface RecentWorkspace extends Recent {
  readonly workspace: { readonly configPath: Uri };
}

export interface RecentFile extends Recent {
  readonly fileUri: Uri;
}

export type RecentEntry = RecentFolder | RecentWorkspace | RecentFile;

/** The list, the most recent first, as `_workbench.getRecentlyOpened` answers it. */
export interface RecentlyOpened {
  readonly workspaces: readonly (RecentFolder | RecentWorkspace)[];
  readonly files: readonly RecentFile[];
}

/**
 * What `_workbench.getRecentlyOpened` answered, as a list. It throws when the answer isn't
 * shaped as one, and an entry that isn't shaped as one is left out.
 */
export function parseRecent(value: unknown): RecentlyOpened {
  const { workspaces, files } = (value ?? {}) as { workspaces?: unknown; files?: unknown };
  if (!Array.isArray(workspaces) || !Array.isArray(files)) {
    throw new Error('the list has no workspaces and files');
  }
  return {
    workspaces: workspaces.filter(
      (entry): entry is RecentFolder | RecentWorkspace =>
        isUri(entry?.folderUri) || isUri(entry?.workspace?.configPath),
    ),
    files: files.filter((entry): entry is RecentFile => isUri(entry?.fileUri)),
  };
}

function isUri(value: unknown): value is Uri {
  const uri = value as Partial<Uri> | undefined;
  return (
    typeof uri?.scheme === 'string' &&
    typeof uri.authority === 'string' &&
    typeof uri.path === 'string'
  );
}

/**
 * An entry's label: the path with `home` as `~`, a workspace and a remote marked as VS Code
 * marks them, cut in the middle at 120 characters, with each underscore doubled so that it
 * isn't read as a mnemonic.
 */
export function recentLabel(entry: RecentEntry, home: string): string {
  const text = entry.label || describe(entry, home);
  return cutMiddle(text, MAX_LABEL).replaceAll('_', '__');
}

function describe(entry: RecentEntry, home: string): string {
  let label: string;
  let uri: Uri;
  if ('workspace' in entry) {
    uri = entry.workspace.configPath;
    // VS Code's own: the file's path less its extension, then "(Workspace)".
    const path = uri.path.endsWith(WORKSPACE_EXTENSION)
      ? uri.path.slice(0, -WORKSPACE_EXTENSION.length)
      : uri.path;
    label = `${pathLabel({ ...uri, path }, home)} (Workspace)`;
  } else {
    uri = 'folderUri' in entry ? entry.folderUri : entry.fileUri;
    label = pathLabel(uri, home);
  }
  const remote = entry.remoteAuthority ?? (uri.scheme === REMOTE ? uri.authority : undefined);
  return remote ? `${label} [${remoteName(remote)}]` : label;
}

const REMOTE = 'vscode-remote';

/** A local path with `home` as `~`; a remote's path as it is, since its home isn't known here. */
function pathLabel(uri: Uri, home: string): string {
  if (uri.scheme === 'file') {
    if (home !== '' && home !== '/') {
      const base = home.replace(/\/+$/, '');
      if (uri.path === base) {
        return '~';
      }
      if (uri.path.startsWith(`${base}/`)) {
        return `~${uri.path.slice(base.length)}`;
      }
    }
    return uri.path;
  }
  if (uri.scheme === REMOTE) {
    return uri.path;
  }
  return `${uri.scheme}://${uri.authority}${uri.path}`;
}

/**
 * The remotes' suffixes as their extensions' label formatters write them, which an
 * extension can't read: the kind, and the host where the authority names one readably.
 */
const REMOTES: Readonly<Record<string, { readonly name: string; readonly host: boolean }>> = {
  'ssh-remote': { name: 'SSH', host: true },
  wsl: { name: 'WSL', host: true },
  tunnel: { name: 'Tunnel', host: true },
  'dev-container': { name: 'Dev Container', host: false },
  'attached-container': { name: 'Container', host: false },
  codespaces: { name: 'Codespaces', host: false },
};

function remoteName(authority: string): string {
  const plus = authority.indexOf('+');
  const kind = plus < 0 ? authority : authority.slice(0, plus);
  const rest = plus < 0 ? '' : authority.slice(plus + 1);
  const known = REMOTES[kind];
  if (known === undefined) {
    return authority;
  }
  return known.host && rest !== '' ? `${known.name}: ${rest}` : known.name;
}

/** `text` cut to `max` characters, with what's cut out of its middle as `…`, as VS Code cuts. */
function cutMiddle(text: string, max: number): string {
  if (text.length <= max) {
    return text;
  }
  const head = Math.floor((max - 1) / 2);
  const tail = max - 1 - head;
  return `${text.slice(0, head)}…${text.slice(text.length - tail)}`;
}

/**
 * Open Recent's entries as items: the first ten folders and workspaces, a separator, the
 * first ten files and a separator. An entry has the same id for as long as the extension
 * runs, so a click on an id from an older layout opens that entry or nothing.
 */
export class RecentItems {
  readonly #home: string;
  readonly #separators: readonly [number, number];
  readonly #ids = new Map<string, number>();
  #next: number;

  /** Ids are given from `firstId` up. */
  constructor(firstId: number, home: string) {
    this.#home = home;
    this.#separators = [firstId, firstId + 1];
    this.#next = firstId + 2;
  }

  items(list: RecentlyOpened): MenuNode[] {
    // An entry the list has twice is shown once, since its id is its own.
    const seen = new Set<number>();
    const firstTen = (entries: readonly RecentEntry[]) =>
      entries
        .slice(0, MAX_ENTRIES)
        .map((entry) => this.#item(entry))
        .filter((item) => !seen.has(item.id) && seen.add(item.id));
    const workspaces = firstTen(list.workspaces);
    const files = firstTen(list.files);
    const [afterWorkspaces, afterFiles] = this.#separators;
    return [
      ...workspaces,
      { id: afterWorkspaces, separator: true },
      ...files,
      { id: afterFiles, separator: true },
    ];
  }

  #item(entry: RecentEntry): MenuNode {
    const [kind, uri, command, args] =
      'folderUri' in entry
        ? (['folder', entry.folderUri, OPEN_FOLDER, [entry.folderUri]] as const)
        : 'workspace' in entry
          ? ([
              'workspace',
              entry.workspace.configPath,
              OPEN_FOLDER,
              [entry.workspace.configPath],
            ] as const)
          : ([
              'file',
              entry.fileUri,
              WINDOW_OPEN,
              [[{ fileUri: entry.fileUri }], { remoteAuthority: entry.remoteAuthority ?? null }],
            ] as const);
    const key = `${kind} ${uri.scheme}://${uri.authority}${uri.path}`;
    let id = this.#ids.get(key);
    if (id === undefined) {
      id = this.#next++;
      this.#ids.set(key, id);
    }
    return { id, label: recentLabel(entry, this.#home), command, args };
  }
}

/**
 * What to run when `command` fails: a recent file that `_files.windowOpen` can't open is
 * opened with `vscode.open`, which leaves the list as it was.
 */
export function fallbackOf(
  command: string,
  args: readonly unknown[],
): [string, unknown[]] | undefined {
  if (command !== WINDOW_OPEN) {
    return undefined;
  }
  const uri = (args[0] as { fileUri?: unknown }[] | undefined)?.[0]?.fileUri;
  return uri === undefined ? undefined : [OPEN, [uri]];
}
