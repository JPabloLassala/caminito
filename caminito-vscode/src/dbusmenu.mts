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
 * Serves a menubar whose top-level items are `menus`, under a root of id 0. `run` is given
 * an item's command and arguments when the item is clicked, after the reply to `Event` is
 * sent.
 */
export class DbusMenu extends dbusInterface.Interface {
  #byId = new Map<number, MenuNode>();
  #revision = 1;
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
   * Serves `menus` from now on, and tells the client what changed: `LayoutUpdated` with a
   * new revision when an item came or went or moved, else `ItemsPropertiesUpdated` when
   * only properties changed, else nothing.
   */
  update(menus: readonly MenuNode[]): void {
    const before = this.#byId;
    const after = byId(menus);
    this.#byId = after;
    if (shape(before.get(0)!) !== shape(after.get(0)!)) {
      this.#revision++;
      this.LayoutUpdated();
      return;
    }
    const updated: [number, Record<string, Variant>][] = [];
    const removed: [number, string[]][] = [];
    for (const [id, node] of after) {
      const [old, now] = [properties(before.get(id)!), properties(node)];
      const changed = Object.keys(now).filter(
        (name) => JSON.stringify(old[name]) !== JSON.stringify(now[name]),
      );
      const gone = Object.keys(old).filter((name) => !(name in now));
      if (changed.length > 0) {
        updated.push([id, variants(now, changed)]);
      }
      if (gone.length > 0) {
        removed.push([id, gone.sort()]);
      }
    }
    if (updated.length > 0 || removed.length > 0) {
      this.ItemsPropertiesUpdated(updated, removed);
    }
  }

  /** The whole tree under `parentId`, whatever the depth asked for: the importer fetches it at once. */
  GetLayout(parentId: number, _depth: number, propertyNames: string[]): [number, Layout] {
    const parent = this.#byId.get(parentId);
    if (!parent) {
      throw new DBusError('org.freedesktop.DBus.Error.InvalidArgs', `no item ${parentId}`);
    }
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

  /** The signal that the tree changed, with the new revision and the root as its parent. */
  LayoutUpdated(): [number, number] {
    return [this.#revision, 0];
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

/** The tree's ids, which a layout reply has to be sent again for when they change. */
function shape(node: MenuNode): string {
  return `${node.id}(${childrenOf(node).map(shape).join(',')})`;
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
