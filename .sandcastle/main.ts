// Ships an epic's tickets, --parallel at a time, each in a Docker sandbox of its own, as /ship
// does by hand: an agent builds the ticket on a branch named after it, a second one reviews the
// branch with /code-review, and the host merges it into master and resolves the ticket in Linear.
//
//   tools/sandcastle start --epic <epic> [--max <n>] [--parallel <n>] [--no-merge]
//     [--implement-model <model>] [--review-model <model>]
//
// The tickets are the epic's frontier, narrowed to ready-for-agent implementation tickets (see
// linear.ts). Merges go one at a time: a branch that master moved under is first caught up in its
// sandbox by an agent. The run stops when the frontier is empty, after --max tickets, when an
// agent fails, or when a merge can't be made; it finishes the tickets it holds first. A ticket
// the agent couldn't finish goes back to Todo with a comment.

import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { parseArgs } from "node:util";
import * as sandcastle from "@ai-hero/sandcastle";
import { docker } from "@ai-hero/sandcastle/sandboxes/docker";
import { Linear, type Ticket } from "./linear.ts";

const IMAGE = "camino-sandcastle";

const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    epic: { type: "string" },
    max: { type: "string", default: "10" },
    parallel: { type: "string", default: "3" },
    "no-merge": { type: "boolean", default: false },
    "implement-model": { type: "string", default: "claude-sonnet-5-5" },
    "review-model": { type: "string", default: "claude-opus-5-5" },
  },
});
if (positionals[0] !== "start" || !values.epic) {
  console.error(
    "usage: tools/sandcastle start --epic <epic> [--max <n>] [--parallel <n>] [--no-merge] [--implement-model <model>] [--review-model <model>]",
  );
  process.exit(2);
}
const epic = values.epic;
const max = Number(values.max);
const parallel = Math.max(1, Number(values.parallel));
const merge = !values["no-merge"];

// The Linear key stays out of .sandcastle/.env, whose every key Sandcastle passes to the sandbox.
try {
  process.loadEnvFile(".sandcastle/host.env");
} catch {}
if (!process.env.LINEAR_API_KEY) {
  console.error("LINEAR_API_KEY is unset: put it in .sandcastle/host.env (see host.env.example)");
  process.exit(2);
}
const linear = new Linear(process.env.LINEAR_API_KEY);
if (!(await linear.milestoneExists(epic))) {
  console.error(`The project camino has no milestone ${epic}`);
  process.exit(2);
}

// Builds share one target directory and one registry, so a ticket doesn't rebuild Smithay.
const cache = join(homedir(), ".cache", "camino-sandcastle");
mkdirSync(join(cache, "target"), { recursive: true });
mkdirSync(join(cache, "cargo-registry"), { recursive: true });
const provider = docker({
  imageName: IMAGE,
  mounts: [
    { hostPath: join(cache, "target"), sandboxPath: "~/target" },
    { hostPath: join(cache, "cargo-registry"), sandboxPath: "~/.cargo/registry" },
  ],
  env: { CARGO_TARGET_DIR: "/home/agent/target" },
});
const implementer = sandcastle.claudeCode(values["implement-model"], { effort: "high" });
const reviewer = sandcastle.claudeCode(values["review-model"], { effort: "medium" });

// What the implementer reports, inside <result> tags.
interface Result {
  status: "done" | "in-review" | "blocked";
  met: number[];
  decisions: string[];
  left?: string;
  note?: string;
}

const parseResult = (stdout: string): Result | undefined => {
  const matches = [...stdout.matchAll(/<result>([\s\S]*?)<\/result>/g)];
  const last = matches.at(-1)?.[1];
  if (!last) return undefined;
  try {
    const result = JSON.parse(last) as Result;
    return ["done", "in-review", "blocked"].includes(result.status) ? result : undefined;
  } catch {
    return undefined;
  }
};

const git = (...args: string[]) => execFileSync("git", args, { encoding: "utf8" }).trim();

// Ticks the met acceptance criteria, numbered as the ticket's checkboxes are, from 1, and
// appends the Done section /ship writes.
const resolvedDescription = (ticket: Ticket, result: Result, mergeCommit?: string) => {
  let box = 0;
  const ticked = ticket.description.replace(/^(\s*[-*] )\[ \]/gm, (whole, bullet: string) =>
    result.met.includes(++box) ? `${bullet}[x]` : whole,
  );
  const date = new Date().toISOString().slice(0, 10);
  const bullets = [...result.decisions, mergeCommit ? `**Merge**: ${mergeCommit}` : "**Merge**: not made"];
  return `${ticked}\n\n## Done (${date})\n\n${bullets.map((b) => `- ${b}`).join("\n")}\n`;
};

// Sandcastle runs ``!`command` `` blocks in a prompt after it puts the arguments in, so text
// from Linear is kept from running as one.
const inert = (text: string) => text.replaceAll("!`", "! `");

// The tickets the run holds, handed back to Todo if the run is stopped, as with Ctrl-C.
const claimed = new Set<Ticket>();
for (const signal of ["SIGINT", "SIGTERM"] as const) {
  process.prependListener(signal, () => {
    for (const ticket of claimed) {
      console.error(`\nStopped: handing ${ticket.identifier} back to Todo.`);
      try {
        linear.releaseSync(ticket);
      } catch {
        console.error(`Couldn't release ${ticket.identifier}: set it to Todo, unassigned, by hand.`);
      }
    }
    claimed.clear();
    // Before a sandbox exists, Sandcastle has no handler of its own to exit with.
    if (process.listenerCount(signal) === 1) process.exit(130);
  });
}

// Merges run one at a time, each after the one before has finished.
let mergeQueue: Promise<unknown> = Promise.resolve();
const oneAtATime = <T>(work: () => Promise<T>): Promise<T> => {
  const turn = mergeQueue.then(work);
  mergeQueue = turn.catch(() => {});
  return turn;
};

// Catches the branch up with master in its sandbox, if master moved, and merges it into master.
// Returns the merge commit, or what's left to do instead.
const mergeBranch = async (
  ticket: Ticket,
  branch: string,
  sandbox: sandcastle.Sandbox,
): Promise<{ commit?: string; left?: string }> => {
  if (git("rev-parse", "--abbrev-ref", "HEAD") !== "master" || git("status", "--porcelain")) {
    return { left: `merge branch \`${branch}\` into master: the main checkout wasn't on a clean master.` };
  }
  const caughtUp = async () => (await sandbox.exec("git merge-base --is-ancestor master HEAD")).exitCode === 0;
  if (!(await caughtUp())) {
    console.log(`${ticket.identifier}: catching up with master`);
    await sandbox.run({
      name: "catch-up",
      maxIterations: 1,
      agent: reviewer,
      promptFile: ".sandcastle/catchup-prompt.md",
      promptArgs: { ID: ticket.identifier, BRANCH: branch, BODY: inert(ticket.description) },
    });
    const dirty = (await sandbox.exec("git status --porcelain")).stdout.trim();
    if (!(await caughtUp()) || dirty) {
      return { left: `merge branch \`${branch}\` into master: the agent couldn't catch it up with master.` };
    }
  }
  try {
    git("merge", "--no-ff", branch, "-m", `Merge branch '${branch}' (${ticket.identifier})`);
    return { commit: git("rev-parse", "--short", "HEAD") };
  } catch {
    git("merge", "--abort");
    return { left: `merge branch \`${branch}\` into master: it conflicts.` };
  }
};

const tried = new Set<string>();
let shipped = 0;
let inFlight = 0;
let stopping = false;

// Resolved, and replaced, each time a ticket finishes, for the workers waiting on the frontier.
let finished!: () => void;
let aTicketFinished = new Promise<void>((resolve) => (finished = resolve));
const signalFinished = () => {
  const resolve = finished;
  aTicketFinished = new Promise<void>((r) => (finished = r));
  resolve();
};

// Builds, reviews and merges one ticket, and resolves it in Linear. Returns whether it shipped.
const ship = async (ticket: Ticket): Promise<boolean> => {
  const id = ticket.identifier;
  const branch = id.toLowerCase();
  console.log(`\n=== ${id}: ${ticket.title} ===\n`);
  const docs =
    (await linear.documents(ticket)).map((d) => `## ${d.title}\n\n${d.content}`).join("\n\n") || "(none)";
  await linear.claim(ticket);
  claimed.add(ticket);

  const sandbox = await sandcastle.createSandbox({ branch, baseBranch: "master", sandbox: provider });
  let result: Result | undefined;
  let merged: { commit?: string; left?: string } = {};
  let failed = false;
  try {
    const implement = await sandbox.run({
      name: "implementer",
      maxIterations: 1,
      agent: implementer,
      promptFile: ".sandcastle/implement-prompt.md",
      promptArgs: {
        ID: id,
        TITLE: inert(ticket.title),
        BODY: inert(ticket.description),
        COMMENTS: inert(ticket.comments.map((c) => `**${c.author}**: ${c.body}`).join("\n\n") || "(none)"),
        DOCS: inert(docs),
      },
    });
    result = parseResult(implement.stdout);
    if (!implement.commits.length && result?.status !== "blocked") {
      result = { status: "blocked", met: [], decisions: [], note: result?.note ?? "The agent made no commits." };
    }
    if (result && result.status !== "blocked") {
      await sandbox.run({
        name: "reviewer",
        maxIterations: 1,
        agent: reviewer,
        promptFile: ".sandcastle/review-prompt.md",
        promptArgs: { ID: id, BRANCH: branch, BODY: inert(ticket.description), DOCS: inert(docs) },
      });
      merged = merge
        ? await oneAtATime(() => mergeBranch(ticket, branch, sandbox))
        : { left: `merge branch \`${branch}\` into master.` };
    }
  } catch (error) {
    // An agent that fails, as one that isn't logged in, blocks the ticket instead of the run's
    // stopping with the ticket claimed. It would fail on the next ticket too, so the run stops.
    failed = true;
    stopping = true;
    const message = error instanceof Error ? error.message : String(error);
    result = { status: "blocked", met: [], decisions: [], note: `The agent failed: ${message.trim()}` };
  } finally {
    // Sandcastle keeps a worktree with uncommitted changes; the branch has all the work there is.
    const { preservedWorktreePath } = await sandbox.close();
    if (preservedWorktreePath) {
      console.log(`${id}: removing ${preservedWorktreePath}, with changes the agent didn't commit:`);
      console.log(execFileSync("git", ["-C", preservedWorktreePath, "status", "--short"], { encoding: "utf8" }));
      git("worktree", "remove", "--force", preservedWorktreePath);
    }
  }

  if (!result || result.status === "blocked") {
    const note = result?.note ?? "The agent's report had no valid <result> block.";
    console.log(`${id} is blocked: ${note}`);
    await linear.comment(ticket, `Sandcastle couldn't finish this ticket on branch \`${branch}\`: ${note}`);
    await linear.release(ticket);
    claimed.delete(ticket);
    return false;
  }

  // The branch goes once its worktree has, as git won't delete a branch checked out in one.
  if (merged.commit) git("branch", "-d", branch);
  // A merge that couldn't be made would fail for the next ticket too.
  if (merge && merged.left) stopping = true;

  await linear.setDescription(ticket, resolvedDescription(ticket, result, merged.commit));
  claimed.delete(ticket);
  const left = [result.status === "in-review" ? result.left : undefined, merged.left].filter(Boolean);
  if (left.length) {
    await linear.setState(ticket, "In Review");
    await linear.comment(ticket, `Left: ${left.join(" And ")}`);
  } else {
    await linear.setState(ticket, "Done");
  }
  console.log(`${id}: ${left.length ? "In Review" : "Done"}${merged.commit ? `, merged as ${merged.commit}` : ""}`);
  return true;
};

// One of --parallel workers, each taking the next frontier ticket no one has tried. A worker
// that finds the frontier empty while others still hold tickets waits for one to finish, as a
// finished ticket can unblock others.
const worker = async () => {
  while (!stopping && shipped + inFlight < max) {
    const frontier = await linear.frontier(epic);
    if (stopping || shipped + inFlight >= max) break;
    const ticket = frontier.find((t) => !tried.has(t.identifier));
    if (!ticket) {
      if (!inFlight) break;
      await aTicketFinished;
      continue;
    }
    tried.add(ticket.identifier);
    inFlight++;
    try {
      if (await ship(ticket)) shipped++;
    } finally {
      inFlight--;
      signalFinished();
    }
  }
};

await Promise.all(Array.from({ length: parallel }, worker));
console.log(stopping ? `\nThe run stopped.` : `\nThe ${epic} frontier is empty, or --max is reached.`);
console.log(`Shipped ${shipped} ticket(s) of ${epic}.`);
