// VS Code's menu table and the rule that fits it to the running editor (ADR 0019).
//
// The table is a snapshot of VS Code's own native menubar, made by `npm run snapshot` and
// committed in menu/table.json. The annotations, written by hand in menu/annotations.json,
// say what a snapshot can't, keyed by command id.

import type { MenuNode, Toggle } from './dbusmenu.mts';
import type { Shortcut } from './keybindings.mts';

/** The table, as menu/table.json holds it. */
export interface Table {
  /** The VS Code it was made from, as its `package.json` and `product.json` say. */
  readonly version: string;
  readonly commit: string;
  /** The menubar's menus, in order. */
  readonly menus: readonly TableSubmenu[];
  /** The snapshot's keybinding labels, by command id, for the fallback of keybinding labels (120). */
  readonly keybindings: Readonly<Record<string, TableKeybinding>>;
}

/** An entry of a menu. */
export type TableItem = TableAction | TableSubmenu | TableSeparator;

/** An item. Its label has `_` before its mnemonic and `__` for an underscore. */
export interface TableAction {
  readonly label: string;
  readonly command: string;
  /** What the command is run with; none when the table doesn't say. */
  readonly args?: readonly unknown[];
}

/** A menu: `submenu` is VS Code's id for it. */
export interface TableSubmenu {
  readonly label: string;
  readonly submenu: string;
  readonly items: readonly TableItem[];
}

/** A line between groups of items. */
export interface TableSeparator {
  readonly separator: true;
}

/** A keybinding label as VS Code's menubar has it. */
export interface TableKeybinding {
  /** Electron's accelerator, or VS Code's label where Electron has none (a chord). */
  readonly label: string;
  /** The form keybindings.json uses. */
  readonly userSettingsLabel?: string;
  /** False when `label` isn't an accelerator. */
  readonly isNative?: boolean;
}

/** What's said by hand of a command's items. */
export interface Annotation {
  /** A `product.json` key the items are shown only with. */
  readonly product?: string;
  /** The setting the items' checkmark mirrors. */
  readonly check?: Condition;
  /** The setting that puts the items on in their radio group. */
  readonly radio?: Condition;
  /** The workspaces each of the items is shown in, by its label. An item not named is always shown. */
  readonly workspace?: Readonly<Record<string, readonly Workspace[]>>;
}

/**
 * A window's workspace, as VS Code's `workbenchState` context key names it: no folder, a
 * folder, or a multi-root workspace.
 */
export type Workspace = 'empty' | 'folder' | 'workspace';

/**
 * What a checkmark or radio item is on for: a setting's value is, isn't or is one of
 * values, or all or any of other conditions.
 */
export type Condition =
  | { readonly setting: string; readonly is: unknown }
  | { readonly setting: string; readonly not: unknown }
  | { readonly setting: string; readonly in: readonly unknown[] }
  | { readonly all: readonly Condition[] }
  | { readonly any: readonly Condition[] };

/** The annotations, by command id. */
export type Annotations = Readonly<Record<string, Annotation>>;

/** What the running editor has, which the table is fitted to. */
export interface Running {
  /** Whether the editor lists a command. */
  hasCommand(command: string): boolean;
  /** The editor's own `product.json`. */
  readonly product: Readonly<Record<string, unknown>>;
  /** A setting's value, or undefined when it can't be read. */
  setting(name: string): unknown;
  readonly workspace: Workspace;
  /** A command's shortcut, or undefined for none. */
  shortcut(command: string): Shortcut | undefined;
  /** What's put before the table's item of `command`, whether that item is shown or not. */
  before?(command: string): readonly MenuNode[];
}

/**
 * The menus the running editor gets: each item whose command it doesn't list, whose
 * `product.json` key it lacks or whose workspace isn't the window's is dropped, then each
 * menu left empty, and then the separators are collapsed, so that none is first, last or
 * beside another. An item whose setting can be read has its checkmark or radio state, and
 * an item whose command has a shortcut has it. What `running.before` gives is put in
 * first, so its separators are collapsed too.
 *
 * Every entry of the table has the same id whatever is dropped: the root is 0 and the rest
 * are numbered depth first.
 */
export function fit(
  menus: readonly TableSubmenu[],
  annotations: Annotations,
  running: Running,
): MenuNode[] {
  const shown = (item: TableAction) => {
    const annotation = annotations[item.command];
    const product = annotation?.product;
    const workspaces = annotation?.workspace?.[item.label];
    return (
      running.hasCommand(item.command) &&
      (product === undefined || !!running.product[product]) &&
      (workspaces === undefined || workspaces.includes(running.workspace))
    );
  };
  const toggle = (item: TableAction): Toggle | undefined => {
    const annotation = annotations[item.command];
    const [type, condition] = annotation?.check
      ? (['checkmark', annotation.check] as const)
      : annotation?.radio
        ? (['radio', annotation.radio] as const)
        : [];
    const state = condition && holds(condition, running.setting);
    return type && state !== undefined ? { type, state } : undefined;
  };
  let next = 1;
  const items = (from: readonly TableItem[]): MenuNode[] => {
    const nodes: MenuNode[] = [];
    for (const item of from) {
      const id = next++;
      if ('separator' in item) {
        nodes.push({ id, separator: true });
      } else if ('submenu' in item) {
        const children = items(item.items);
        if (children.length > 0) {
          nodes.push({ id, label: item.label, children });
        }
      } else {
        nodes.push(...(running.before?.(item.command) ?? []));
        if (shown(item)) {
          const state = toggle(item);
          const shortcut = running.shortcut(item.command);
          nodes.push({
            id,
            ...item,
            ...(state && { toggle: state }),
            ...(shortcut && { shortcut }),
          });
        }
      }
    }
    return collapse(nodes);
  };
  return items(menus);
}

/** Whether `condition` holds, or undefined when a setting it reads can't be read. */
function holds(condition: Condition, setting: (name: string) => unknown): boolean | undefined {
  if ('all' in condition || 'any' in condition) {
    const parts = 'all' in condition ? condition.all : condition.any;
    const results = parts.map((part) => holds(part, setting));
    if (results.includes(undefined)) {
      return undefined;
    }
    return 'all' in condition ? results.every(Boolean) : results.some(Boolean);
  }
  const value = setting(condition.setting);
  // A value of another type than the condition's is one that can't be read.
  const expected =
    'is' in condition ? [condition.is] : 'not' in condition ? [condition.not] : condition.in;
  if (value === undefined || !expected.some((other) => typeof other === typeof value)) {
    return undefined;
  }
  return 'is' in condition
    ? value === condition.is
    : 'not' in condition
      ? value !== condition.not
      : condition.in.includes(value);
}

/** How many entries `menus` has in all, which is the highest id `fit` gives. */
export function entriesIn(menus: readonly TableItem[]): number {
  return menus.reduce(
    (count, item) => count + 1 + ('items' in item ? entriesIn(item.items) : 0),
    0,
  );
}

/** The settings the annotations' checkmarks and radio items read. */
export function settingsOf(annotations: Annotations): string[] {
  const names = new Set<string>();
  const walk = (condition: Condition) => {
    if ('all' in condition) {
      condition.all.forEach(walk);
    } else if ('any' in condition) {
      condition.any.forEach(walk);
    } else {
      names.add(condition.setting);
    }
  };
  for (const annotation of Object.values(annotations)) {
    for (const condition of [annotation.check, annotation.radio]) {
      if (condition) {
        walk(condition);
      }
    }
  }
  return [...names].sort();
}

/** The commands of every item under `items`. */
export function commandsOf(items: readonly TableItem[]): string[] {
  return items.flatMap((item) =>
    'command' in item ? [item.command] : 'items' in item ? commandsOf(item.items) : [],
  );
}

/** `items` with no separator first, last or beside another. */
export function collapse<T extends MenuNode | TableItem>(items: readonly T[]): T[] {
  const out: T[] = [];
  for (const item of items) {
    const last = out[out.length - 1];
    if ('separator' in item && (last === undefined || 'separator' in last)) {
      continue;
    }
    out.push(item);
  }
  if (out.length > 0 && 'separator' in out[out.length - 1]!) {
    out.pop();
  }
  return out;
}
