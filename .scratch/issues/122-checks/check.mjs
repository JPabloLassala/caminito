// Ticket 122's checks, run in the session up.sh left up:  in.sh <work> node check.mjs <work> <command>
//
//   hosts                    each live extension host, as its driver described it
//   offers                   the offer's notifications each window shows
//   drive <folder> <code>    runs code in the host of the window with that folder
//   race <tries> <gap ms>    two windows, on <work>/a and <work>/b, focused the gap apart
//   agents                   the agents window, opened beside a and b: its workspace file, and
//                            a claim from it and then one from a, with the offer on
//   profile                  the editor started again with one window, on <work>/c in a
//                            profile of its own, after the offer was made
//   state                    the settings file's two settings, and who holds the bus name
//
// It reads a window's notifications from its page, through the DevTools port editor.sh opens.
import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const here = import.meta.dirname;
const [work, command, ...args] = process.argv.slice(2);
const driver = path.join(work, 'driver');
const OFFER = 'caminito.globalMenu.offerToHideMenuBar';
const VISIBILITY = 'window.menuBarVisibility';
const TEXT = "The panel now shows this window's menus.";
const PORT = 9322;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export function hosts() {
  return fs
    .readdirSync(driver)
    .filter((name) => /^\d+$/.test(name) && fs.existsSync(`/proc/${name}`))
    .map((name) => JSON.parse(fs.readFileSync(path.join(driver, name, 'host.json'), 'utf8')));
}

/** The live host of the window that `match` picks, waited for. */
export async function host(match, wait = 30000) {
  // The agents window has a workspace file, and may have another window's folder.
  const test =
    typeof match === 'function'
      ? match
      : (h) => h.workspaceFile === null && h.folders.some((f) => path.basename(f) === match);
  for (let waited = 0; waited < wait; waited += 100) {
    const found = hosts().filter(test);
    if (found.length === 1) {
      return found[0];
    }
    await sleep(100);
  }
  throw new Error(`no one host for ${match}`);
}

let scripts = 0;
export async function drive(pid, code) {
  const name = path.join(driver, String(pid), `${Date.now()}-${String(scripts++).padStart(4, '0')}`);
  fs.writeFileSync(`${name}.tmp`, code);
  fs.renameSync(`${name}.tmp`, `${name}.js`);
  for (let waited = 0; waited < 20000; waited += 20) {
    if (fs.existsSync(`${name}.out`)) {
      const result = JSON.parse(fs.readFileSync(`${name}.out`, 'utf8'));
      if ('error' in result) {
        throw new Error(result.error);
      }
      return result.ok;
    }
    await sleep(20);
  }
  throw new Error(`host ${pid} didn't run the script`);
}

export async function pages() {
  const targets = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
  return targets.filter((target) => target.type === 'page');
}

export function evaluate(page, expression) {
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(page.webSocketDebuggerUrl);
    socket.onerror = () => reject(new Error(`no DevTools socket for ${page.title}`));
    socket.onopen = () =>
      socket.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression, returnByValue: true } }));
    socket.onmessage = (event) => {
      const reply = JSON.parse(event.data);
      if (reply.id === 1) {
        socket.close();
        resolve(reply.result?.result?.value);
      }
    };
  });
}

const NOTIFICATIONS = `[...document.querySelectorAll('.notification-list-item')].map((item) => ({
  message: item.querySelector('.notification-list-item-message')?.textContent ?? '',
  buttons: [...item.querySelectorAll('.notification-list-item-buttons-container .monaco-button')].map((b) => b.textContent),
}))`;

/** The offer's notifications on screen, by window title. */
export async function offers() {
  const shown = {};
  for (const page of await pages()) {
    const all = (await evaluate(page, NOTIFICATIONS)) ?? [];
    shown[page.title] = all.filter((one) => one.message.startsWith(TEXT));
  }
  return shown;
}

/** Presses a button of the offer in the window whose title has `title`. */
export async function press(title, button) {
  const page = (await pages()).find((one) => one.title.includes(title));
  return evaluate(
    page,
    `(() => { const b = [...document.querySelectorAll('.notification-list-item-buttons-container .monaco-button')]
      .find((b) => b.textContent === ${JSON.stringify(button)}); b?.click(); return b !== undefined; })()`,
  );
}

/** The session bus's messages from line `from` of its log on, each with its arguments. */
export function bus(from = 0) {
  const messages = [];
  for (const line of fs.readFileSync(path.join(work, 'out/bus.log'), 'utf8').split('\n').slice(from)) {
    if (/^(method call|signal|method return|error)/.test(line)) {
      messages.push({
        time: Number(/time=([\d.]+)/.exec(line)?.[1]) * 1000,
        sender: /sender=(\S+)/.exec(line)?.[1],
        member: /member=(\w+)/.exec(line)?.[1],
        arguments: [],
      });
    } else if (messages.length > 0 && line.trim() !== '') {
      messages.at(-1).arguments.push(line.trim());
    }
  }
  return messages;
}
export const busLines = () => fs.readFileSync(path.join(work, 'out/bus.log'), 'utf8').split('\n').length - 1;

function busctl(...call) {
  try {
    return execFileSync('busctl', ['--user', ...call], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
  } catch {
    return null;
  }
}
const DBUS = ['org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus'];
const pidOf = (name) => Number(busctl('call', ...DBUS, 'GetConnectionUnixProcessID', 's', name)?.split(' ')[1]) || null;

/** The pid of the host that holds the offer's bus name for an editor's main process. */
export const holder = (parentPid) => pidOf(`org.mate.Caminito.GlobalMenuExtension.pid${parentPid}`);

export function settings() {
  const all = JSON.parse(fs.readFileSync(path.join(work, 'user/User/settings.json'), 'utf8'));
  return { [OFFER]: all[OFFER], [VISIBILITY]: all[VISIBILITY] };
}

const SET = (key, value) =>
  `await vscode.workspace.getConfiguration().update(${JSON.stringify(key)}, ${JSON.stringify(value)}, true)`;
const GET = (key) => `return vscode.workspace.getConfiguration().get(${JSON.stringify(key)})`;

/** Sets a user setting from one host, and waits until each host given reads it. */
export async function set(key, value, from, readers = [from]) {
  await drive(from.pid, SET(key, value));
  for (const reader of readers) {
    while ((await drive(reader.pid, GET(key))) !== value) {
      await sleep(50);
    }
  }
}

export function focus(gap, ...titles) {
  return execFileSync('python3', [path.join(here, 'focus.py'), String(gap), ...titles], { encoding: 'utf8' });
}
export const windows = () => execFileSync('python3', [path.join(here, 'focus.py'), 'list'], { encoding: 'utf8' });

/** What the bus and the windows saw since a mark: the claims, the name's requests, the offers. */
export async function seen(mark, since) {
  const messages = bus(mark);
  const of = (member) =>
    messages.filter((m) => m.member === member).map((m) => ({ pid: pidOf(m.sender), at: Math.round(m.time - since) }));
  const shown = await offers();
  return {
    claims: of('ClaimGlobalMenu'),
    nameRequests: messages
      .filter((m) => m.member === 'RequestName' && m.arguments[0]?.includes('GlobalMenuExtension'))
      .map((m) => ({ pid: pidOf(m.sender), at: Math.round(m.time - since) })),
    offers: Object.fromEntries(Object.entries(shown).map(([title, list]) => [title, list.length])),
    ...settings(),
  };
}

/**
 * Two windows, each with a new host, the offer on and the menubar showing, focused `gap` ms
 * apart. A try is a race when both hosts asked for the name.
 */
async function race(tries, gap) {
  const tally = { tries: 0, raced: 0, oneOffer: 0 };
  for (let attempt = 1; attempt <= tries; attempt++) {
    let [a, b] = [await host('a'), await host('b')];
    await set(OFFER, false, a, [a, b]);
    await set(VISIBILITY, 'classic', a, [a, b]);
    const old = [a.pid, b.pid];
    for (const one of [a, b]) {
      await drive(one.pid, `setTimeout(() => vscode.commands.executeCommand('workbench.action.reloadWindow'), 50)`);
    }
    const fresh = (name) => (h) => !old.includes(h.pid) && h.folders.some((f) => path.basename(f) === name);
    [a, b] = [await host(fresh('a')), await host(fresh('b'))];
    await sleep(2500);
    for (const one of [a, b]) {
      await drive(one.pid, `await vscode.commands.executeCommand('notifications.clearAll')`);
    }
    await set(OFFER, true, a, [a, b]);
    await sleep(1000);
    const before = Object.values(await offers()).flat().length;
    const list = windows().split('\n');
    const order = list.find((line) => line.startsWith('*'))?.includes('a - ') ? ['b - ', 'a - '] : ['a - ', 'b - '];
    const mark = busLines();
    const since = Date.now();
    focus(gap, ...order);
    await sleep(2500);
    const result = await seen(mark, since);
    const named = (pid) => (pid === a.pid ? 'a' : pid === b.pid ? 'b' : String(pid));
    const count = Object.values(result.offers).reduce((sum, n) => sum + n, 0);
    const raced = new Set(result.nameRequests.map((r) => r.pid)).size === 2;
    tally.tries++;
    tally.raced += raced ? 1 : 0;
    tally.oneOffer += count === 1 ? 1 : 0;
    console.log(
      JSON.stringify({
        attempt,
        order: order.map((title) => title[0]).join(' then '),
        offersBefore: before,
        claims: result.claims.map((c) => `${named(c.pid)}@${c.at}`),
        nameRequests: result.nameRequests.map((r) => `${named(r.pid)}@${r.at}`),
        holder: named(holder(a.parentPid)),
        offers: result.offers,
        offerSetting: result[OFFER],
      }),
    );
  }
  console.log(JSON.stringify(tally));
}

const isAgents = (h) => h.workspaceFile?.endsWith('/agent-sessions.code-workspace') ?? false;
const RELOAD = `setTimeout(() => vscode.commands.executeCommand('workbench.action.reloadWindow'), 50)`;
const CLEAR = `await vscode.commands.executeCommand('notifications.clearAll')`;
// A change of the active editor, which claims in a focused window.
const NEW_FILE = `await vscode.commands.executeCommand('workbench.action.files.newUntitledFile')`;

async function agents() {
  let [a, b, agent] = [await host('a'), await host('b'), await host(isAgents)];
  const userDir = path.join(work, 'user/User');
  console.log(
    JSON.stringify({
      workspaceFile: agent.workspaceFile,
      isTheSettingsDirectorys: agent.workspaceFile === `file://${userDir}/agent-sessions.code-workspace`,
      globalStorage: agent.globalStorage,
      underTheDefaultProfiles: path.dirname(agent.globalStorage) === path.join(userDir, 'globalStorage'),
      profiles: JSON.parse(fs.readFileSync(path.join(userDir, 'globalStorage/storage.json'), 'utf8')).userDataProfiles,
    }),
  );
  // New hosts for a and b, so nobody holds the name and the agents window's host could get it.
  await set(OFFER, false, a, [a, b, agent]);
  await set(VISIBILITY, 'classic', a, [a, b, agent]);
  const old = [a.pid, b.pid];
  await drive(a.pid, RELOAD);
  await drive(b.pid, RELOAD);
  const fresh = (name) => (h) => !old.includes(h.pid) && !isAgents(h) && h.folders.some((f) => path.basename(f) === name);
  [a, b] = [await host(fresh('a')), await host(fresh('b'))];
  await sleep(2500);
  focus(0, 'Agents');
  await sleep(1000);
  for (const one of [a, b, agent]) {
    await drive(one.pid, CLEAR);
  }
  await set(OFFER, true, a, [a, b, agent]);
  await sleep(1000);
  const named = (list) => list.map((c) => `${{ [a.pid]: 'a', [b.pid]: 'b', [agent.pid]: 'agents' }[c.pid] ?? c.pid}@${c.at}`);
  const report = (what, result) =>
    console.log(
      JSON.stringify({
        what,
        claims: named(result.claims),
        nameRequests: named(result.nameRequests),
        holder: named([{ pid: holder(a.parentPid), at: 0 }])[0].split('@')[0],
        offers: result.offers,
        offerSetting: result[OFFER],
      }),
    );
  let [mark, since] = [busLines(), Date.now()];
  await drive(agent.pid, NEW_FILE);
  await sleep(2500);
  report('a claim from the agents window', await seen(mark, since));
  [mark, since] = [busLines(), Date.now()];
  focus(0, 'a - ');
  await sleep(2500);
  report('then a focused', await seen(mark, since));
}

const SETTINGS_FILE = `await vscode.commands.executeCommand('workbench.action.openSettingsJson');
  return vscode.window.activeTextEditor?.document.uri.toString()`;

/**
 * After the offer was made: the editor quits and starts again with one window, in a profile
 * of its own, so its host is new and nobody holds the name. Then the offer is turned on
 * from that window, it claims, and Hide Menu Bar is pressed.
 */
async function profile() {
  const running = hosts();
  if (running.length > 0) {
    await drive(running[0].pid, `setTimeout(() => vscode.commands.executeCommand('workbench.action.quit'), 50)`);
    while (fs.existsSync(`/proc/${running[0].parentPid}`)) {
      await sleep(100);
    }
  }
  const made = settings();
  let [mark, since] = [busLines(), Date.now()];
  spawn(path.join(here, 'editor.sh'), [work, '--profile', 'other', '--new-window', path.join(work, 'c')], {
    detached: true,
    stdio: 'ignore',
  }).unref();
  const c = await host('c');
  await sleep(4000);
  const report = (what, result, more = {}) =>
    console.log(
      JSON.stringify({
        what,
        claims: result.claims.map((claim) => `${claim.pid === c.pid ? 'c' : claim.pid}@${claim.at}`),
        nameRequests: result.nameRequests.map((r) => `${r.pid === c.pid ? 'c' : r.pid}@${r.at}`),
        offers: result.offers,
        settingsFile: settings(),
        ...more,
      }),
    );
  report('a window of another profile, the offer made before', await seen(mark, since), {
    settingsFileBefore: made,
    hostReads: {
      [OFFER]: await drive(c.pid, GET(OFFER)),
      [VISIBILITY]: await drive(c.pid, GET(VISIBILITY)),
    },
    globalStorage: c.globalStorage,
    windowsSettingsFile: await drive(c.pid, SETTINGS_FILE),
  });
  await set(OFFER, true, c);
  await sleep(1000);
  [mark, since] = [busLines(), Date.now()];
  await drive(c.pid, NEW_FILE);
  await sleep(2500);
  report('the offer turned on from it, then a claim', await seen(mark, since));
  const pressed = await press('c - ', 'Hide Menu Bar');
  await sleep(2000);
  report('Hide Menu Bar pressed', await seen(busLines(), Date.now()), {
    pressed,
    hostReads: { [VISIBILITY]: await drive(c.pid, GET(VISIBILITY)) },
  });
}

if (command === 'hosts') {
  console.log(JSON.stringify(hosts(), null, 1));
} else if (command === 'offers') {
  console.log(JSON.stringify(await offers(), null, 1));
} else if (command === 'drive') {
  console.log(JSON.stringify(await drive((await host(args[0])).pid, args[1])));
} else if (command === 'agents') {
  await agents();
} else if (command === 'profile') {
  await profile();
} else if (command === 'race') {
  await race(Number(args[0]), Number(args[1]));
} else if (command === 'state') {
  const parents = [...new Set(hosts().map((h) => h.parentPid))];
  console.log(JSON.stringify({ ...settings(), holders: Object.fromEntries(parents.map((p) => [p, holder(p)])) }));
}
