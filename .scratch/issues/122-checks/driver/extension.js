// Ticket 122's checks: each extension host gets a directory of its own under
// $CAMINITO_122_DRIVER, named by its pid. It writes what it is to host.json there: its
// parent's pid, the window's workspace file and folders, and this extension's global storage,
// which has the same parent as any other extension's. It then runs the scripts dropped
// there, one at a time, as the body of an async function of `vscode`, `sleep`, `require` and
// `context`, and writes what each returned beside it. Each change of the window's focus is
// logged with a timestamp to 'focus-events'.
const vscode = require('vscode');
const fs = require('node:fs');
const path = require('node:path');

const AsyncFunction = Object.getPrototypeOf(async () => {}).constructor;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

exports.activate = (context) => {
  const dir = process.env.CAMINITO_122_DRIVER;
  if (!dir) {
    return;
  }
  const mine = path.join(dir, String(process.pid));
  fs.mkdirSync(mine, { recursive: true });
  fs.writeFileSync(
    path.join(mine, 'host.json'),
    JSON.stringify({
      pid: process.pid,
      parentPid: process.ppid,
      started: Date.now(),
      workspaceFile: vscode.workspace.workspaceFile?.toString() ?? null,
      folders: (vscode.workspace.workspaceFolders ?? []).map((folder) => folder.uri.fsPath),
      globalStorage: context.globalStorageUri.fsPath,
      focused: vscode.window.state.focused,
    }),
  );
  context.subscriptions.push(
    vscode.window.onDidChangeWindowState((state) =>
      fs.appendFileSync(path.join(dir, 'focus-events'), `${Date.now()} ${process.pid} ${state.focused}\n`),
    ),
  );
  let busy = false;
  const timer = setInterval(async () => {
    if (busy) {
      return;
    }
    const name = fs.readdirSync(mine).filter((file) => file.endsWith('.js')).sort()[0];
    if (name === undefined) {
      return;
    }
    busy = true;
    const file = path.join(mine, name);
    let result;
    try {
      const code = fs.readFileSync(file, 'utf8');
      fs.renameSync(file, `${file}.taken`);
      const run = new AsyncFunction('vscode', 'sleep', 'require', 'context', code);
      result = { ok: (await run(vscode, sleep, require, context)) ?? null };
    } catch (err) {
      result = { error: String(err.stack ?? err) };
    }
    fs.writeFileSync(`${file}.out.tmp`, JSON.stringify(result));
    fs.renameSync(`${file}.out.tmp`, file.replace(/\.js$/, '.out'));
    busy = false;
  }, 20);
  context.subscriptions.push({ dispose: () => clearInterval(timer) });
};
