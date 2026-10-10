// The keybinding labels of the menu's items (ADR 0020, 120): the rule that picks a
// command's one binding from the default keybindings and the user's file, and dbusmenu's
// `shortcut` for it.
//
// Keys are written as keybindings.json has them, which VS Code's own parser reads: the
// modifiers `ctrl`, `shift`, `alt` and `meta` (or `win` or `cmd`), each followed by `+` or
// `-`, then the key, and a space before each further press of a chord.

import type { MenuEntry } from './dbusmenu.mts';
import type { TableKeybinding } from './menu-table.mts';

/** An entry of a keybindings file. A command that starts with `-` is a removal. */
export interface Binding {
  /** The keys, or '' for a removal that names none. */
  readonly key: string;
  readonly command: string;
  readonly when?: string;
}

/** One press of a key, with its modifiers. */
interface Press {
  readonly ctrl: boolean;
  readonly shift: boolean;
  readonly alt: boolean;
  readonly meta: boolean;
  /** The key, in lower case, under the name VS Code writes for a US keyboard. */
  readonly key: string;
}

/** A shortcut as dbusmenu's `shortcut` has it: for each press, its modifiers and a GDK key name. */
export type Shortcut = NonNullable<MenuEntry['shortcut']>;

/**
 * The GDK key name of each key VS Code 1.140 can bind, by the name keybindings.json gives
 * it: the user settings labels of `mappings` in VS Code's `src/vs/base/common/keyCodes.ts`,
 * line 495 at the 1.140.0 tag.
 */
export const GDK_KEYS: Readonly<Record<string, string>> = {
  ...Object.fromEntries([...'abcdefghijklmnopqrstuvwxyz0123456789'].map((key) => [key, key])),
  enter: 'Return',
  escape: 'Escape',
  backspace: 'BackSpace',
  tab: 'Tab',
  space: 'space',
  '-': 'minus',
  '=': 'equal',
  '[': 'bracketleft',
  ']': 'bracketright',
  '\\': 'backslash',
  ';': 'semicolon',
  "'": 'apostrophe',
  '`': 'grave',
  ',': 'comma',
  '.': 'period',
  '/': 'slash',
  capslock: 'Caps_Lock',
  ...Object.fromEntries(Array.from({ length: 24 }, (_, i) => [`f${i + 1}`, `F${i + 1}`])),
  scrolllock: 'Scroll_Lock',
  pausebreak: 'Pause',
  insert: 'Insert',
  home: 'Home',
  pageup: 'Page_Up',
  delete: 'Delete',
  end: 'End',
  pagedown: 'Page_Down',
  right: 'Right',
  left: 'Left',
  down: 'Down',
  up: 'Up',
  numlock: 'Num_Lock',
  numpad_divide: 'KP_Divide',
  numpad_multiply: 'KP_Multiply',
  numpad_subtract: 'KP_Subtract',
  numpad_add: 'KP_Add',
  ...Object.fromEntries(Array.from({ length: 10 }, (_, i) => [`numpad${i}`, `KP_${i}`])),
  numpad_decimal: 'KP_Decimal',
  numpad_separator: 'KP_Separator',
  contextmenu: 'Menu',
  clear: 'Clear',
  audiovolumemute: 'XF86AudioMute',
  audiovolumeup: 'XF86AudioRaiseVolume',
  audiovolumedown: 'XF86AudioLowerVolume',
  mediatracknext: 'XF86AudioNext',
  mediatrackprevious: 'XF86AudioPrev',
  mediastop: 'XF86AudioStop',
  mediaplaypause: 'XF86AudioPlay',
  launchmediaplayer: 'XF86AudioMedia',
  launchmail: 'XF86Mail',
  launchapp2: 'XF86Calculator',
  browsersearch: 'XF86Search',
  browserhome: 'XF86HomePage',
  browserback: 'XF86Back',
  browserforward: 'XF86Forward',
};

/** The keys VS Code 1.140 can bind that are knowingly left with no GDK name, and why. */
export const WITHOUT_GDK_KEY: Readonly<Record<string, string>> = {
  unknown: 'VS Code has no key behind it',
  keyincomposition: 'it stands for an input method at work, not a key',
  oem_102: 'the key beside the left Shift is a different keysym on each layout',
  abnt_c1: 'a Brazilian key whose keysym depends on the layout',
  abnt_c2: 'a Brazilian key whose keysym depends on the layout',
  oem_8: 'a key whose keysym depends on the layout',
  ctrl: 'a modifier alone, which GTK draws no accelerator for',
  shift: 'a modifier alone, which GTK draws no accelerator for',
  alt: 'a modifier alone, which GTK draws no accelerator for',
  meta: 'a modifier alone, which GTK draws no accelerator for',
};

/** The other names VS Code reads for a key (its general labels), by the name it writes. */
const ALIASES: Readonly<Record<string, string>> = {
  oem_minus: '-',
  oem_plus: '=',
  oem_4: '[',
  oem_6: ']',
  oem_5: '\\',
  oem_1: ';',
  oem_7: "'",
  oem_3: '`',
  oem_comma: ',',
  oem_period: '.',
  oem_2: '/',
};

/**
 * The binding of each command that VS Code's own menu would show, by command (ADR 0020):
 * the user's removals apply first; a binding stops counting when a later entry with no
 * `when` gives its keys to another command; then of what's left for a command, the only
 * binding, else the last with no `when`, else the last.
 *
 * `when` clauses are compared as written, with their spaces evened out, where VS Code
 * compares them parsed.
 */
export function labelsOf(
  defaults: readonly Binding[],
  user: readonly Binding[],
): Map<string, string> {
  const removals = user.filter((binding) => isRemoval(binding));
  const removed = (binding: Binding) =>
    removals.some((removal) => {
      if (removal.command.slice(1) !== binding.command) {
        return false;
      }
      // A removal's presses match the binding's first ones, as `_isTargetedForRemoval` has
      // it in VS Code's `src/vs/platform/keybinding/common/keybindingResolver.ts`, line 91
      // at the 1.140.0 tag.
      const keys = removal.key === '' ? [] : parse(removal.key);
      const presses = parse(binding.key);
      if (keys === undefined || presses === undefined) {
        return false;
      }
      if (keys.some((press, i) => presses[i] === undefined || same(press, presses[i]!) === false)) {
        return false;
      }
      const when = normal(removal.when);
      return when === undefined || when === normal(binding.when);
    });
  const live: { binding: Binding; presses: Press[] }[] = [];
  for (const binding of [
    ...defaults.filter((binding) => !isRemoval(binding) && !removed(binding)),
    ...user.filter((binding) => !isRemoval(binding)),
  ]) {
    const presses = parse(binding.key);
    if (presses === undefined) {
      continue;
    }
    if (normal(binding.when) === undefined) {
      // A key and a chord that starts with it take each other's place too, as
      // `_addKeyPress` has it in keybindingResolver.ts, line 177 at the 1.140.0 tag.
      for (let i = live.length - 1; i >= 0; i--) {
        const other = live[i]!;
        if (other.binding.command !== binding.command && eitherPrefixes(other.presses, presses)) {
          live.splice(i, 1);
        }
      }
    }
    live.push({ binding, presses });
  }
  const byCommand = new Map<string, Binding[]>();
  for (const { binding } of live) {
    byCommand.set(binding.command, [...(byCommand.get(binding.command) ?? []), binding]);
  }
  const labels = new Map<string, string>();
  for (const [command, bindings] of byCommand) {
    const always = bindings.filter((binding) => normal(binding.when) === undefined);
    const chosen = always.length > 0 ? always[always.length - 1]! : bindings[bindings.length - 1]!;
    labels.set(command, chosen.key);
  }
  return labels;
}

/**
 * The labels when the default keybindings can't be read (ADR 0020): the snapshot's, and for
 * each command the user's file names, the last binding it gives the command, or none if it
 * only removes one.
 */
export function fallbackLabelsOf(
  snapshot: Readonly<Record<string, TableKeybinding>>,
  user: readonly Binding[],
): Map<string, string> {
  const labels = new Map<string, string>();
  for (const [command, keybinding] of Object.entries(snapshot)) {
    if (keybinding.userSettingsLabel !== undefined) {
      labels.set(command, keybinding.userSettingsLabel);
    }
  }
  for (const binding of user) {
    if (isRemoval(binding)) {
      const command = binding.command.slice(1);
      if (!user.some((other) => other.command === command && parse(other.key) !== undefined)) {
        labels.delete(command);
      }
    } else if (parse(binding.key) !== undefined) {
      labels.set(binding.command, binding.key);
    }
  }
  return labels;
}

/**
 * dbusmenu's `shortcut` for keys as keybindings.json writes them: for each press, in order,
 * the modifiers in the order `Control`, `Alt`, `Shift`, `Super`, then the key's GDK name. A
 * chord has one entry per press, which the applet's carried patch draws one after another
 * (145, ADR 0020). A key with no GDK name, a bare scan code among them, has none, and nor has
 * a chord with such a press.
 */
export function shortcutOf(key: string): Shortcut | undefined {
  const presses = parse(key);
  if (presses === undefined) {
    return undefined;
  }
  const shortcut: string[][] = [];
  for (const press of presses) {
    const name = GDK_KEYS[press.key];
    if (name === undefined) {
      return undefined;
    }
    const modifiers = [
      press.ctrl && 'Control',
      press.alt && 'Alt',
      press.shift && 'Shift',
      press.meta && 'Super',
    ].filter((modifier) => modifier !== false);
    shortcut.push([...modifiers, name]);
  }
  return shortcut;
}

/** The bindings of a parsed keybindings file. An entry not shaped as one is skipped. */
export function bindingsOf(file: unknown): Binding[] {
  if (!Array.isArray(file)) {
    return [];
  }
  const bindings: Binding[] = [];
  for (const entry of file as unknown[]) {
    if (typeof entry !== 'object' || entry === null) {
      continue;
    }
    const { key, command, when } = entry as Record<string, unknown>;
    if (typeof command !== 'string' || command === '' || command === '-') {
      continue;
    }
    if (typeof key !== 'string' && !(key === undefined && command.startsWith('-'))) {
      continue;
    }
    bindings.push({ key: key ?? '', command, ...(typeof when === 'string' && { when }) });
  }
  return bindings;
}

/**
 * Parses JSON with comments and trailing commas, as VS Code's settings and keybindings files
 * are written. Throws as `JSON.parse` does on anything else.
 */
export function parseJsonc(text: string): unknown {
  let out = '';
  let i = 0;
  while (i < text.length) {
    const c = text[i]!;
    if (c === '"') {
      const start = i++;
      while (i < text.length && text[i] !== '"') {
        i += text[i] === '\\' ? 2 : 1;
      }
      out += text.slice(start, ++i);
    } else if (c === '/' && text[i + 1] === '/') {
      while (i < text.length && text[i] !== '\n') {
        i++;
      }
    } else if (c === '/' && text[i + 1] === '*') {
      const end = text.indexOf('*/', i + 2);
      i = end === -1 ? text.length : end + 2;
    } else if (c === ',' && /^\s*[\]}]/.test(text.slice(i + 1))) {
      i++;
    } else {
      out += c;
      i++;
    }
  }
  return JSON.parse(out);
}

function isRemoval(binding: Binding): boolean {
  return binding.command.startsWith('-');
}

/** A `when` clause with its spaces evened out, or undefined for none. */
function normal(when: string | undefined): string | undefined {
  const trimmed = when?.replace(/\s+/g, ' ').trim();
  return trimmed === '' ? undefined : trimmed;
}

/** Whether one of two sequences of presses starts with the other. */
function eitherPrefixes(a: readonly Press[], b: readonly Press[]): boolean {
  const length = Math.min(a.length, b.length);
  return a.slice(0, length).every((press, i) => same(press, b[i]!));
}

function same(a: Press, b: Press): boolean {
  return (
    a.ctrl === b.ctrl &&
    a.shift === b.shift &&
    a.alt === b.alt &&
    a.meta === b.meta &&
    a.key === b.key
  );
}

/**
 * The presses of keys as keybindings.json writes them, or undefined for keys VS Code can't
 * read. As `_readModifiers` reads them in VS Code's `src/vs/base/common/keybindingParser.ts`,
 * line 11 at the 1.140.0 tag.
 */
function parse(keys: string): Press[] | undefined {
  const presses: Press[] = [];
  let rest = keys.toLowerCase().trim();
  while (rest !== '') {
    let ctrl = false;
    let shift = false;
    let alt = false;
    let meta = false;
    for (let matched = true; matched; ) {
      const modifier = /^(ctrl|shift|alt|meta|win|cmd)[+-]/.exec(rest);
      matched = modifier !== null;
      if (modifier) {
        const name = modifier[1]!;
        ctrl ||= name === 'ctrl';
        shift ||= name === 'shift';
        alt ||= name === 'alt';
        meta ||= name === 'meta' || name === 'win' || name === 'cmd';
        rest = rest.slice(modifier[0].length);
      }
    }
    const space = rest.indexOf(' ');
    const key = space > 0 ? rest.slice(0, space) : rest;
    rest = space > 0 ? rest.slice(space).trim() : '';
    const named = ALIASES[key] ?? key;
    const known = named in GDK_KEYS || named in WITHOUT_GDK_KEY || /^\[[^\]]+\]$/.test(named);
    if (!known) {
      return undefined;
    }
    presses.push({ ctrl, shift, alt, meta, key: named });
  }
  return presses.length > 0 ? presses : undefined;
}
