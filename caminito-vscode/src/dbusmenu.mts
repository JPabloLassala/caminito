// The `com.canonical.dbusmenu` server: the smallest one the global menu applet's importer
// works with (ticket 04 of vscode-global-menu). The importer calls it synchronously from
// inside mate-panel, so nothing here makes the panel wait on the editor.

import { DBusError, Variant, interface as dbusInterface } from '@jellybrick/dbus-next';

/** A menu or an item, as the menu is written. A menu has `children`, an item a `command`. */
export interface MenuNode {
  /** The label, with `_` before its mnemonic. */
  readonly label: string;
  readonly command?: string;
  readonly children?: readonly MenuNode[];
}

/** An item of the served layout: `(ia{sv}av)`. */
type Layout = [number, Record<string, Variant>, Variant[]];

interface Numbered {
  readonly id: number;
  readonly node: MenuNode;
  readonly children: readonly Numbered[];
}

const INTERFACE = 'com.canonical.dbusmenu';

/**
 * Serves a menubar whose top-level items are `menus`. The root is id 0 and the rest are
 * numbered depth first. `run` is given an item's command when the item is clicked, after
 * the reply to `Event` is sent.
 */
export class DbusMenu extends dbusInterface.Interface {
  readonly #byId = new Map<number, Numbered>();
  readonly #run: (command: string) => void;

  constructor(menus: readonly MenuNode[], run: (command: string) => void) {
    super(INTERFACE);
    // In 0.11.4 the base class's constructor sets these as own fields, which hide what
    // `configureMembers()` put on the prototype, so nothing would be served. Deleting
    // them uncovers the prototype's. To be reported to JellyBrick/node-dbus-next.
    delete this.$properties;
    delete this.$methods;
    delete this.$signals;
    let next = 0;
    const number = (node: MenuNode): Numbered => {
      const id = next++;
      const numbered = { id, node, children: (node.children ?? []).map(number) };
      this.#byId.set(id, numbered);
      return numbered;
    };
    number({ label: '', children: menus });
    this.#run = run;
  }

  /** The whole tree under `parentId`, whatever the depth asked for: the importer fetches it at once. */
  GetLayout(parentId: number, _depth: number, propertyNames: string[]): [number, Layout] {
    const parent = this.#byId.get(parentId);
    if (!parent) {
      throw new DBusError('org.freedesktop.DBus.Error.InvalidArgs', `no item ${parentId}`);
    }
    return [1, layout(parent, propertyNames)];
  }

  /** Answered before the command runs, since the panel waits for the reply. */
  Event(id: number, eventId: string, _data: Variant, _timestamp: number): void {
    const command = this.#byId.get(id)?.node.command;
    if (eventId === 'clicked' && command !== undefined) {
      setImmediate(() => this.#run(command));
    }
  }

  /** Nothing is built late, so nothing ever needs an update. */
  AboutToShow(_id: number): boolean {
    return false;
  }
}

DbusMenu.configureMembers({
  methods: {
    GetLayout: { inSignature: 'iias', outSignature: 'u(ia{sv}av)' },
    Event: { inSignature: 'isvu' },
    AboutToShow: { inSignature: 'i', outSignature: 'b' },
  },
});

function layout(item: Numbered, propertyNames: string[]): Layout {
  const children = item.children.map(
    (child) => new Variant('(ia{sv}av)', layout(child, propertyNames)),
  );
  return [item.id, properties(item, propertyNames), children];
}

/**
 * An item's properties in alphabetical order: with `label` before `children-display` the
 * importer gives a submenu a stray `action`.
 */
function properties(item: Numbered, propertyNames: string[]): Record<string, Variant> {
  const all: Record<string, Variant> = {};
  if (item.children.length > 0) {
    all['children-display'] = new Variant('s', 'submenu');
  }
  if (item.id !== 0) {
    all['label'] = new Variant('s', item.node.label);
  }
  const sorted: Record<string, Variant> = {};
  for (const name of Object.keys(all).sort()) {
    if (propertyNames.length === 0 || propertyNames.includes(name)) {
      sorted[name] = all[name]!;
    }
  }
  return sorted;
}
