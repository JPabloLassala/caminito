// VS Code's menu table and the rule that fits it to the running editor (ADR 0019).
//
// The table is a snapshot of VS Code's own native menubar, made by `npm run snapshot` and
// committed in menu/table.json. The annotations, written by hand in menu/annotations.json,
// say what a snapshot can't, keyed by command id.

import type { MenuNode } from './dbusmenu.mts';

/** The table, as menu/table.json holds it. */
export interface Table {
  /** The VS Code it was made from, as its `package.json` and `product.json` say. */
  readonly version: string;
  readonly commit: string;
  /** The menubar's menus, in order. */
  readonly menus: readonly TableSubmenu[];
  /** The snapshot's keybinding labels, by command id, for ticket 120's fallback. */
  readonly keybindings: Readonly<Record<string, TableKeybinding>>;
}

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
}

export type Annotations = Readonly<Record<string, Annotation>>;

/** What the running editor has, which the table is fitted to. */
export interface Running {
  /** Whether the editor lists a command. */
  hasCommand(command: string): boolean;
  /** The editor's own `product.json`. */
  readonly product: Readonly<Record<string, unknown>>;
}

/**
 * The menus the running editor gets: each item whose command it doesn't list or whose
 * `product.json` key it lacks is dropped, then each menu left empty, and then the
 * separators are collapsed, so that none is first, last or beside another.
 */
export function fit(
  menus: readonly TableSubmenu[],
  annotations: Annotations,
  running: Running,
): MenuNode[] {
  const shown = (item: TableAction) => {
    const product = annotations[item.command]?.product;
    return (
      running.hasCommand(item.command) && (product === undefined || !!running.product[product])
    );
  };
  const items = (from: readonly TableItem[]): MenuNode[] => {
    const nodes: MenuNode[] = [];
    for (const item of from) {
      if ('separator' in item) {
        nodes.push({ separator: true });
      } else if ('submenu' in item) {
        const children = items(item.items);
        if (children.length > 0) {
          nodes.push({ label: item.label, children });
        }
      } else if (shown(item)) {
        nodes.push(item);
      }
    }
    return collapse(nodes);
  };
  return items(menus);
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
