// What the snapshot command makes of VS Code's `lastKnownMenubarData` (ADR 0019): the table,
// with what belongs to the run stripped, the check of the annotations against it, and the
// weekly canary's diff of two tables' structure (123).

import { collapse } from '../src/menu-table.mts';
import type {
  Annotations,
  Table,
  TableAction,
  TableItem,
  TableKeybinding,
  TableSubmenu,
} from '../src/menu-table.mts';

/** `lastKnownMenubarData`, as VS Code's `IMenubarData` has it. */
export interface MenubarData {
  readonly menus: Readonly<Record<string, { readonly items: readonly MenubarItem[] }>>;
  readonly keybindings: Readonly<Record<string, TableKeybinding | undefined>>;
}

/** An action, a recent entry (with `uri`), a submenu (with `submenu`) or a separator. */
export interface MenubarItem {
  readonly id: string;
  readonly label?: string;
  readonly submenu?: { readonly items: readonly MenubarItem[] };
  readonly uri?: unknown;
  readonly enabled?: boolean;
  readonly checked?: boolean;
}

/** The made-from line of the table. */
export interface Made {
  readonly version: string;
  readonly commit: string;
}

const SEPARATOR = 'vscode.menubar.separator';

/** The profile of a run with a throwaway profile. */
const PROFILE = 'Default';

/** The entries VS Code adds for each profile, under New Window with Profile and Profiles. */
const PROFILE_ENTRIES = [
  'workbench.action.openProfile.',
  'workbench.profiles.actions.profileEntry.',
];

/**
 * The table of snapshots taken in several workspace states, merged in order. What belongs to
 * the run is left out: `enabled`, `checked`, Open Recent's entries, each profile's entry and
 * the profile's name in a label.
 */
export function tableOf(made: Made, snapshots: readonly MenubarData[]): Table {
  const tables = snapshots.map(menusOf);
  const menus = tables.reduce((a, b) => merge(a, b) as TableSubmenu[]);
  const keybindings: Record<string, TableKeybinding> = {};
  for (const snapshot of snapshots) {
    for (const [command, keybinding] of Object.entries(snapshot.keybindings)) {
      if (keybinding !== undefined && !(command in keybindings)) {
        keybindings[command] = keybinding;
      }
    }
  }
  const sorted = Object.fromEntries(
    Object.keys(keybindings)
      .sort()
      .map((command) => [command, keybindings[command]!]),
  );
  return { version: made.version, commit: made.commit, menus, keybindings: sorted };
}

/**
 * The commands of annotations that the table has no item of, and as `<command> "<label>"`
 * each label a workspace annotation names that none of its command's items has.
 */
export function staleAnnotations(table: Table, annotations: Annotations): string[] {
  const labels = new Map<string, Set<string>>();
  const walk = (items: readonly TableItem[]) => {
    for (const item of items) {
      if ('command' in item) {
        labels.set(item.command, (labels.get(item.command) ?? new Set()).add(item.label));
      } else if ('items' in item) {
        walk(item.items);
      }
    }
  };
  walk(table.menus);
  return Object.entries(annotations).flatMap(([command, annotation]) => {
    const has = labels.get(command);
    if (has === undefined) {
      return [command];
    }
    return Object.keys(annotation.workspace ?? {})
      .filter((label) => !has.has(label))
      .map((label) => `${command} ${JSON.stringify(label)}`);
  });
}

/** A label with VS Code's `&&` mnemonic as dbusmenu's `_`, and an underscore doubled. */
export function mnemonic(label: string): string {
  return label.replace(/_/g, '__').replace(/&&/g, '_');
}

function menusOf(snapshot: MenubarData): TableSubmenu[] {
  return Object.entries(snapshot.menus).map(([name, menu]) => ({
    // Every top-level title has its first letter as its mnemonic: "&&File" to "&&Help".
    label: `_${name}`,
    submenu: name,
    items: itemsOf(menu.items),
  }));
}

function itemsOf(items: readonly MenubarItem[]): TableItem[] {
  const out: TableItem[] = [];
  for (const item of items) {
    if (item.id === SEPARATOR) {
      out.push({ separator: true });
    } else if (item.uri !== undefined || PROFILE_ENTRIES.some((p) => item.id.startsWith(p))) {
      continue;
    } else if (item.submenu !== undefined) {
      const items = itemsOf(item.submenu.items);
      if (items.some((child) => !('separator' in child))) {
        out.push({ label: labelOf(item), submenu: item.id, items });
      }
    } else {
      out.push({ label: labelOf(item), command: item.id });
    }
  }
  return collapse(out);
}

function labelOf(item: MenubarItem): string {
  return mnemonic((item.label ?? '').replace(` (${PROFILE})`, ''));
}

/**
 * The items of `a` and `b` in one list: their longest common run in order, with what only
 * one has put where it has it, `a`'s first. An item is the same in both by its command and
 * label, a menu by its id, and any separator matches any other.
 */
export function merge(a: readonly TableItem[], b: readonly TableItem[]): TableItem[] {
  const [ka, kb] = [a.map(key), b.map(key)];
  // lcs[i][j] is the length of the longest common run of a[i..] and b[j..].
  const lcs = Array.from({ length: a.length + 1 }, () => new Array<number>(b.length + 1).fill(0));
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      lcs[i]![j] =
        ka[i] === kb[j] ? lcs[i + 1]![j + 1]! + 1 : Math.max(lcs[i + 1]![j]!, lcs[i]![j + 1]!);
    }
  }
  const out: TableItem[] = [];
  const [onlyA, onlyB]: TableItem[][] = [[], []];
  let [i, j] = [0, 0];
  while (i < a.length && j < b.length) {
    if (ka[i] === kb[j]) {
      out.push(...onlyA.splice(0), ...onlyB.splice(0), both(a[i]!, b[j]!));
      i++;
      j++;
    } else if (lcs[i + 1]![j]! >= lcs[i]![j + 1]!) {
      onlyA.push(a[i++]!);
    } else {
      onlyB.push(b[j++]!);
    }
  }
  out.push(...onlyA, ...a.slice(i), ...onlyB, ...b.slice(j));
  return collapse(out);
}

function both(a: TableItem, b: TableItem): TableItem {
  if ('items' in a && 'items' in b) {
    return { ...a, items: merge(a.items, b.items) };
  }
  return a;
}

function key(item: TableItem): string {
  if ('separator' in item) {
    return '-';
  }
  if ('items' in item) {
    return `menu ${item.submenu}`;
  }
  return `item ${(item as TableAction).command} ${item.label}`;
}

/** The lines around a difference that `structureDiff` shows. */
const DIFF_CONTEXT_LINES = 3;

/**
 * What differs between the menus of two tables, as a unified diff of their outlines, or ''
 * when they have the same structure. Their versions, commits and keybinding labels aren't
 * structure. A hunk starts with a bare `@@`.
 */
export function structureDiff(committed: Table, fresh: Table): string {
  const [a, b] = [outline(committed.menus), outline(fresh.menus)];
  // lcs[i][j] is the length of the longest common run of a[i..] and b[j..].
  const lcs = Array.from({ length: a.length + 1 }, () => new Array<number>(b.length + 1).fill(0));
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      lcs[i]![j] =
        a[i] === b[j] ? lcs[i + 1]![j + 1]! + 1 : Math.max(lcs[i + 1]![j]!, lcs[i]![j + 1]!);
    }
  }
  const lines: string[] = [];
  let [i, j] = [0, 0];
  while (i < a.length || j < b.length) {
    if (i < a.length && j < b.length && a[i] === b[j]) {
      lines.push(` ${a[i++]}`);
      j++;
    } else if (j >= b.length || (i < a.length && lcs[i + 1]![j]! >= lcs[i]![j + 1]!)) {
      lines.push(`-${a[i++]}`);
    } else {
      lines.push(`+${b[j++]}`);
    }
  }
  const changed = lines.flatMap((line, at) => (line.startsWith(' ') ? [] : [at]));
  if (changed.length === 0) {
    return '';
  }
  const diff = [
    `--- committed table (VS Code ${committed.version})`,
    `+++ snapshot (VS Code ${fresh.version})`,
  ];
  // The index after the last line shown; a change past it and its context starts a hunk.
  let shown = 0;
  for (const at of changed) {
    const start = Math.max(at - DIFF_CONTEXT_LINES, shown);
    if (shown === 0 || start > shown) {
      diff.push('@@');
    }
    shown = Math.min(at + DIFF_CONTEXT_LINES + 1, lines.length);
    diff.push(...lines.slice(start, shown));
  }
  return `${diff.join('\n')}\n`;
}

/** A line for each menu and item, indented by its depth: its label and its id. */
function outline(items: readonly TableItem[], depth = 0): string[] {
  const indent = '  '.repeat(depth);
  return items.flatMap((item) => {
    if ('separator' in item) {
      return [`${indent}---`];
    }
    if ('items' in item) {
      return [`${indent}${item.label}  > ${item.submenu}`, ...outline(item.items, depth + 1)];
    }
    return [`${indent}${item.label}  ${item.command}`];
  });
}
