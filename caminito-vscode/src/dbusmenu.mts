// The `com.canonical.dbusmenu` server: the smallest one the global menu applet's importer
// works with (ticket 04 of vscode-global-menu). The importer calls it synchronously from
// inside mate-panel, so nothing here makes the panel wait on the editor.

import { DBusError, Variant, interface as dbusInterface } from '@jellybrick/dbus-next';

/** A menu, an item or a separator, as the menu is written. */
export type MenuNode = MenuEntry | MenuSeparator;

/**
 * A menu or an item. A menu has `children`, an item a `command` and maybe its arguments,
 * its checkmark or radio state and its shortcut.
 */
export interface MenuEntry {
  /** The item's id, which stays the same while it's there. The root's is 0. */
  readonly id: number;
  /** The label, with `_` before its mnemonic and `__` for an underscore. */
  readonly label: string;
  readonly command?: string;
  readonly args?: readonly unknown[];
  readonly toggle?: Toggle;
  /** dbusmenu's `shortcut`: for each press, its modifiers and then a GDK key name. */
  readonly shortcut?: readonly (readonly string[])[];
  readonly children?: readonly MenuNode[];
}

/** An item's checkmark or radio state. */
export interface Toggle {
  readonly type: 'checkmark' | 'radio';
  readonly state: boolean;
}

/** A line between groups of items, served as `type` `separator`. */
export interface MenuSeparator {
  readonly id: number;
  readonly separator: true;
}

/** An item of the served layout: `(ia{sv}av)`. */
type Layout = [number, Record<string, Variant>, Variant[]];

/** An item's properties, by name, as their signature and value. */
type Properties = Record<string, [string, unknown]>;

const INTERFACE = 'com.canonical.dbusmenu';

/**
 * How long after a menu's `GetLayout` its items' properties are sent again. The importer
 * drops an `ItemsPropertiesUpdated` for 100 ms after a menu's layout reply reaches it, and
 * reads nothing again afterwards (119).
 */
export const DROPPED_MS = 200;

/**
 * Serves a menubar whose top-level items are `menus`, under a root of id 0. `run` is given
 * an item's command and arguments when the item is clicked, after the reply to `Event` is
 * sent.
 */
export class DbusMenu extends dbusInterface.Interface {
  #byId = new Map<number, MenuNode>();
  #revision = 1;
  /** When each menu's layout was last asked for, by the menu's id. */
  #answered = new Map<number, number>();
  /** The properties to send again, by item, and the one-shot that sends them. */
  #late = new Map<number, Set<string>>();
  #resending: ReturnType<typeof setTimeout> | undefined;
  readonly #run: (command: string, args: readonly unknown[]) => void;

  constructor(
    menus: readonly MenuNode[],
    run: (command: string, args: readonly unknown[]) => void,
  ) {
    super(INTERFACE);
    // In 0.11.4 the base class's constructor sets these as own fields, which hide what
    // `configureMembers()` put on the prototype, so nothing would be served. Deleting
    // them uncovers the prototype's. To be reported to JellyBrick/node-dbus-next.
    delete this.$properties;
    delete this.$methods;
    delete this.$signals;
    this.#byId = byId(menus);
    this.#run = run;
  }

  /**
   * Serves `menus` from now on, and tells the client what changed: `ItemsPropertiesUpdated`
   * for the properties of the items that stay, then a `LayoutUpdated` with a new revision
   * for each menu whose own items came, went or moved. The importer fetches again only the
   * menu a `LayoutUpdated` names, and none under it.
   */
  update(menus: readonly MenuNode[]): void {
    const before = this.#byId;
    const after = byId(menus);
    this.#byId = after;
    const updated: [number, Record<string, Variant>][] = [];
    const removed: [number, string[]][] = [];
    const relaid: number[] = [];
    const parents = parentsOf(after);
    const now = performance.now();
    for (const [id, node] of after) {
      const was = before.get(id);
      // A new item comes with its menu's layout.
      if (was === undefined) {
        continue;
      }
      if (idsOf(was) !== idsOf(node)) {
        relaid.push(id);
      }
      const [old, current] = [properties(was), properties(node)];
      const changed = Object.keys(current).filter(
        (name) => JSON.stringify(old[name]) !== JSON.stringify(current[name]),
      );
      const gone = Object.keys(old).filter((name) => !(name in current));
      if (changed.length > 0) {
        updated.push([id, variants(current, changed)]);
      }
      if (gone.length > 0) {
        removed.push([id, gone.sort()]);
      }
      // The importer may drop this one: its menu's layout was asked for a moment ago.
      const answered = this.#answered.get(parents.get(id) ?? 0);
      if (changed.length + gone.length > 0 && answered !== undefined) {
        if (now - answered < DROPPED_MS) {
          const names = this.#late.get(id) ?? new Set();
          [...changed, ...gone].forEach((name) => names.add(name));
          this.#late.set(id, names);
          this.#resending ??= setTimeout(() => this.#resend(), answered + DROPPED_MS - now);
        }
      }
    }
    if (updated.length > 0 || removed.length > 0) {
      this.ItemsPropertiesUpdated(updated, removed);
    }
    if (relaid.length > 0) {
      this.#revision++;
      relaid.forEach((parent) => this.LayoutUpdated(parent));
    }
  }

  /** Sends no more: the one-shot that sends properties again is dropped. */
  dispose(): void {
    clearTimeout(this.#resending);
    this.#late.clear();
  }

  /** Sends the properties held back as they are now, or waits again for a menu asked for since. */
  #resend(): void {
    this.#resending = undefined;
    const parents = parentsOf(this.#byId);
    const now = performance.now();
    const updated: [number, Record<string, Variant>][] = [];
    const removed: [number, string[]][] = [];
    let wait = 0;
    for (const [id, names] of this.#late) {
      const node = this.#byId.get(id);
      if (node === undefined) {
        this.#late.delete(id);
        continue;
      }
      const left = (this.#answered.get(parents.get(id) ?? 0) ?? -Infinity) + DROPPED_MS - now;
      if (left > 0) {
        wait = Math.max(wait, left);
        continue;
      }
      this.#late.delete(id);
      const current = properties(node);
      const [there, gone] = [
        [...names].filter((name) => name in current),
        [...names].filter((name) => !(name in current)),
      ];
      if (there.length > 0) {
        updated.push([id, variants(current, there)]);
      }
      if (gone.length > 0) {
        removed.push([id, gone.sort()]);
      }
    }
    if (updated.length > 0 || removed.length > 0) {
      this.ItemsPropertiesUpdated(updated, removed);
    }
    if (this.#late.size > 0) {
      this.#resending = setTimeout(() => this.#resend(), wait);
    }
  }

  /** The whole tree under `parentId`, whatever the depth asked for: the importer fetches it at once. */
  GetLayout(parentId: number, _depth: number, propertyNames: string[]): [number, Layout] {
    const parent = this.#byId.get(parentId);
    if (!parent) {
      throw new DBusError('org.freedesktop.DBus.Error.InvalidArgs', `no item ${parentId}`);
    }
    this.#answered.set(parentId, performance.now());
    return [this.#revision, layout(parent, propertyNames)];
  }

  /** Answered before the command runs, since the panel waits for the reply. */
  Event(id: number, eventId: string, _data: Variant, _timestamp: number): void {
    const node = this.#byId.get(id);
    if (eventId === 'clicked' && node !== undefined && 'command' in node) {
      const { command, args = [] } = node;
      if (command !== undefined) {
        setImmediate(() => this.#run(command, args));
      }
    }
  }

  /** Nothing is built late, so nothing ever needs an update. */
  AboutToShow(_id: number): boolean {
    return false;
  }

  /** The signal that the items of the menu `parent` changed, with the new revision. */
  LayoutUpdated(parent: number): [number, number] {
    return [this.#revision, parent];
  }

  /** The signal that properties changed: the new values, and the names that went back to their defaults. */
  ItemsPropertiesUpdated(
    updated: [number, Record<string, Variant>][],
    removed: [number, string[]][],
  ): [[number, Record<string, Variant>][], [number, string[]][]] {
    return [updated, removed];
  }
}

DbusMenu.configureMembers({
  methods: {
    GetLayout: { inSignature: 'iias', outSignature: 'u(ia{sv}av)' },
    Event: { inSignature: 'isvu' },
    AboutToShow: { inSignature: 'i', outSignature: 'b' },
  },
  signals: {
    LayoutUpdated: { signature: 'ui' },
    ItemsPropertiesUpdated: { signature: 'a(ia{sv})a(ias)' },
  },
});

/** The root, of id 0, and every node under it, by id. */
function byId(menus: readonly MenuNode[]): Map<number, MenuNode> {
  const all = new Map<number, MenuNode>();
  const add = (node: MenuNode) => {
    all.set(node.id, node);
    childrenOf(node).forEach(add);
  };
  add({ id: 0, label: '', children: menus });
  return all;
}

function childrenOf(node: MenuNode): readonly MenuNode[] {
  return 'separator' in node ? [] : (node.children ?? []);
}

/** Each node's parent, by id. */
function parentsOf(all: Map<number, MenuNode>): Map<number, number> {
  const parents = new Map<number, number>();
  for (const [id, node] of all) {
    childrenOf(node).forEach((child) => parents.set(child.id, id));
  }
  return parents;
}

/** The ids of a node's own children, in order. */
function idsOf(node: MenuNode): string {
  return childrenOf(node)
    .map((child) => child.id)
    .join(',');
}

function layout(node: MenuNode, propertyNames: string[]): Layout {
  const children = childrenOf(node).map(
    (child) => new Variant('(ia{sv}av)', layout(child, propertyNames)),
  );
  const all = properties(node);
  const names = Object.keys(all).filter(
    (name) => propertyNames.length === 0 || propertyNames.includes(name),
  );
  return [node.id, variants(all, names), children];
}

/** A node's properties, other than the ones dbusmenu defaults. */
function properties(node: MenuNode): Properties {
  const all: Properties = {};
  if (childrenOf(node).length > 0) {
    all['children-display'] = ['s', 'submenu'];
  }
  if ('separator' in node) {
    all['type'] = ['s', 'separator'];
  } else {
    if (node.id !== 0) {
      all['label'] = ['s', node.label];
    }
    if (node.toggle) {
      all['toggle-type'] = ['s', node.toggle.type];
      all['toggle-state'] = ['i', node.toggle.state ? 1 : 0];
    }
    if (node.shortcut) {
      all['shortcut'] = ['aas', node.shortcut];
    }
  }
  return all;
}

/**
 * `names` of `all` as variants, in alphabetical order: with `label` before
 * `children-display` the importer gives a submenu a stray `action`.
 */
function variants(all: Properties, names: string[]): Record<string, Variant> {
  const sorted: Record<string, Variant> = {};
  for (const name of [...names].sort()) {
    const [signature, value] = all[name]!;
    sorted[name] = new Variant(signature, value);
  }
  return sorted;
}
