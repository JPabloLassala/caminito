// Ticket 121's checks: runs the scripts the check drops in $CAMINITO_121_DRIVER, one at a
// time, as the body of an async function of `vscode`, `sleep` and `require`, and writes what each
// returned beside it. It's how the check changes a setting or the folders at a known time.
// It also logs two events with a timestamp each, for the two questions under "To run": a
// change of the tabs, to 'tabs-events', and the window's focus, to 'focus-events'.
const vscode = require('vscode');
const fs = require('node:fs');
const path = require('node:path');

const AsyncFunction = Object.getPrototypeOf(async () => {}).constructor;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

exports.activate = (context) => {
  const dir = process.env.CAMINITO_121_DRIVER;
  if (!dir) {
    return;
  }
  // One line for each host that started, which a change of workspace state adds to.
  fs.appendFileSync(path.join(dir, 'hosts'), `${Date.now()} ${process.pid}\n`);
  if (vscode.window.tabGroups?.onDidChangeTabs) {
    context.subscriptions.push(
      vscode.window.tabGroups.onDidChangeTabs(() =>
        fs.appendFileSync(path.join(dir, 'tabs-events'), `${Date.now()} ${process.pid}\n`),
      ),
    );
  }
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
    const name = fs.readdirSync(dir).filter((file) => file.endsWith('.js')).sort()[0];
    if (name === undefined) {
      return;
    }
    busy = true;
    const file = path.join(dir, name);
    let result;
    try {
      const code = fs.readFileSync(file, 'utf8');
      // Another window's host may have taken it.
      fs.renameSync(file, `${file}.taken`);
      result = { ok: (await new AsyncFunction('vscode', 'sleep', 'require', code)(vscode, sleep, require)) ?? null };
    } catch (err) {
      if (err.code === 'ENOENT') {
        busy = false;
        return;
      }
      result = { error: String(err.stack ?? err) };
    }
    fs.writeFileSync(`${file}.out.tmp`, JSON.stringify(result));
    fs.renameSync(`${file}.out.tmp`, file.replace(/\.js$/, '.out'));
    busy = false;
  }, 20);
  context.subscriptions.push({ dispose: () => clearInterval(timer) });
};
