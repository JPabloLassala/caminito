// Which keybindings file is the window's (ADR 0020, 120). VS Code doesn't tell an extension
// its window's profile, so it's read from VS Code's own `storage.json`, which is private
// state and may change shape.

import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

/** The default profile's id, as `profileAssociations` names it. */
const DEFAULT_PROFILE = '__default__profile__';

/** The profile VS Code gives the agents window, which uses the default's keybindings. */
const AGENTS_PROFILE = 'agents';

/**
 * The user's keybindings file for a window, from `storage.json` as parsed (undefined when it
 * couldn't be read). `userDir` is the default profile's directory, `<user data>/User`.
 * `workspace` is the window's workspace file or, with none, its first folder, as a URI's
 * string; undefined for an empty window. `newWindowProfile` is `window.newWindowProfile`.
 *
 * The profile tied to the workspace is the window's; with no tie, it's the one
 * `newWindowProfile` names, or else the default. A profile that uses the default's
 * keybindings has the default's file, and so has every window when `storage.json` isn't
 * shaped as expected.
 */
export function keybindingsFileOf(
  userDir: string,
  storage: unknown,
  workspace: string | undefined,
  newWindowProfile: unknown,
): string {
  const fallback = join(userDir, 'keybindings.json');
  if (!isRecord(storage)) {
    return fallback;
  }
  const stored = storage['userDataProfiles'] ?? [];
  const associations = storage['profileAssociations'] ?? {};
  if (!Array.isArray(stored) || !isRecord(associations)) {
    return fallback;
  }
  const workspaces = associations['workspaces'] ?? {};
  if (!isRecord(workspaces)) {
    return fallback;
  }
  // As VS Code reads them: a profile with no name or place is skipped.
  const profiles = (stored as unknown[]).flatMap((profile) => {
    if (!isRecord(profile) || typeof profile['name'] !== 'string' || profile['name'] === '') {
      return [];
    }
    const location = pathOf(profile['location']);
    if (location === undefined) {
      return [];
    }
    const flags = profile['useDefaultFlags'];
    return [
      {
        id: location.split('/').filter(Boolean).pop() ?? '',
        name: profile['name'],
        location,
        defaultKeybindings: isRecord(flags) && flags['keybindings'] === true,
      },
    ];
  });
  const tied = workspace === undefined ? undefined : workspaces[workspace];
  const profile =
    tied === DEFAULT_PROFILE
      ? undefined
      : typeof tied === 'string' && profiles.some((profile) => profile.id === tied)
        ? profiles.find((profile) => profile.id === tied)
        : profiles.find((profile) => profile.name === newWindowProfile);
  if (profile === undefined || profile.defaultKeybindings || profile.id === AGENTS_PROFILE) {
    return fallback;
  }
  return join(profile.location, 'keybindings.json');
}

/** `storage.json`'s place under the default profile's directory. */
export function storageFileOf(userDir: string): string {
  return join(userDir, 'globalStorage', 'storage.json');
}

/** The path of a profile's `location`: a URI as VS Code stores one, or its string. */
function pathOf(location: unknown): string | undefined {
  if (typeof location === 'string') {
    try {
      return fileURLToPath(location);
    } catch {
      return undefined;
    }
  }
  if (isRecord(location) && typeof location['path'] === 'string') {
    return (location['scheme'] ?? 'file') === 'file' ? location['path'] : undefined;
  }
  return undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
