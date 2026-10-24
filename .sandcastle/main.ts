// Ships an epic's tickets one at a time, each in a Docker sandbox of its own, as /ship does by
// hand: an agent builds the ticket on a branch named after it, a second one reviews the branch
// with /code-review, and the host merges it into master and resolves the ticket in Linear.
//
//   tools/sandcastle start --epic <epic> [--max <n>] [--no-merge]
//     [--implement-model <model>] [--review-model <model>]
//
// The tickets are the epic's frontier, narrowed to ready-for-agent implementation tickets (see
// linear.ts). The run stops when the frontier is empty, after --max tickets, or when a merge
// can't be made; a ticket the agent couldn't finish goes back to Todo with a comment.

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
    "no-merge": { type: "boolean", default: false },
    "implement-model": { type: "string", default: "claude-sonnet-5-5" },
    "review-model": { type: "string", default: "claude-opus-5-5" },
  },
});
if (positionals[0] !== "start" || !values.epic) {
  console.error(
    "usage: tools/sandcastle start --epic <epic> [--max <n>] [--no-merge] [--implement-model <model>] [--review-model <model>]",
  );
  process.exit(2);
}
const epic = values.epic;
const max = Number(values.max);
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

const tried = new Set<string>();
let shipped = 0;

while (shipped < max) {
  const ticket = (await linear.frontier(epic)).find((t) => !tried.has(t.identifier));
  if (!ticket) {
    console.log(`\nThe ${epic} frontier is empty.`);
    break;
  }
  tried.add(ticket.identifier);
  const branch = ticket.identifier.toLowerCase();
  console.log(`\n=== ${ticket.identifier}: ${ticket.title} ===\n`);
  await linear.claim(ticket);

  const sandbox = await sandcastle.createSandbox({ branch, baseBranch: "master", sandbox: provider });
  let result: Result | undefined;
  let failed = false;
  try {
    const implement = await sandbox.run({
      name: "implementer",
      maxIterations: 1,
      agent: implementer,
      promptFile: ".sandcastle/implement-prompt.md",
      promptArgs: {
        ID: ticket.identifier,
        TITLE: ticket.title,
        BODY: ticket.description,
        COMMENTS: ticket.comments.map((c) => `**${c.author}**: ${c.body}`).join("\n\n") || "(none)",
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
        promptArgs: { ID: ticket.identifier, BRANCH: branch, BODY: ticket.description },
      });
    }
  } catch (error) {
    // An agent that fails, as one that isn't logged in, blocks the ticket instead of the run's
    // stopping with the ticket claimed. It would fail on the next ticket too, so the run stops.
    failed = true;
    const message = error instanceof Error ? error.message : String(error);
    result = { status: "blocked", met: [], decisions: [], note: `The agent failed: ${message.trim()}` };
  } finally {
    await sandbox.close();
  }

  if (!result || result.status === "blocked") {
    const note = result?.note ?? "The agent's report had no valid <result> block.";
    console.log(`${ticket.identifier} is blocked: ${note}`);
    await linear.comment(ticket, `Sandcastle couldn't finish this ticket on branch \`${branch}\`: ${note}`);
    await linear.release(ticket);
    if (failed) break;
    continue;
  }

  let mergeCommit: string | undefined;
  let mergeLeft: string | undefined;
  if (!merge) {
    mergeLeft = `merge branch \`${branch}\` into master.`;
  } else if (git("rev-parse", "--abbrev-ref", "HEAD") !== "master" || git("status", "--porcelain")) {
    mergeLeft = `merge branch \`${branch}\` into master: the main checkout wasn't on a clean master.`;
  } else {
    try {
      git("merge", "--no-ff", branch, "-m", `Merge branch '${branch}' (${ticket.identifier})`);
      mergeCommit = git("rev-parse", "--short", "HEAD");
      git("branch", "-d", branch);
    } catch {
      git("merge", "--abort");
      mergeLeft = `merge branch \`${branch}\` into master: it conflicts.`;
    }
  }

  await linear.setDescription(ticket, resolvedDescription(ticket, result, mergeCommit));
  const left = [result.status === "in-review" ? result.left : undefined, mergeLeft].filter(Boolean);
  if (left.length) {
    await linear.setState(ticket, "In Review");
    await linear.comment(ticket, `Left: ${left.join(" And ")}`);
  } else {
    await linear.setState(ticket, "Done");
  }
  shipped++;
  console.log(`${ticket.identifier}: ${left.length ? "In Review" : "Done"}${mergeCommit ? `, merged as ${mergeCommit}` : ""}`);
  if (mergeLeft && merge) break;
}

console.log(`\nShipped ${shipped} ticket(s) of ${epic}.`);
