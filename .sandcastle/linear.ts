// What the host side of a Sandcastle run asks of Linear, through its GraphQL API. The sandbox
// never sees the API key: the host claims and resolves the tickets, and the agents only code.

import { execFileSync } from "node:child_process";

const API = "https://api.linear.app/graphql";
const PROJECT = "camino";
const TEAM = "Camino";

export interface Ticket {
  id: string;
  identifier: string;
  title: string;
  description: string;
  createdAt: string;
  comments: { author: string; body: string }[];
}

export interface Doc {
  title: string;
  content: string;
}

interface IssueNode {
  id: string;
  identifier: string;
  title: string;
  description: string | null;
  createdAt: string;
  inverseRelations: { nodes: { type: string; issue: { state: { type: string } } }[] };
  comments: { nodes: { body: string; user: { name: string } | null }[] };
}

export class Linear {
  #key: string;
  #states = new Map<string, string>();

  constructor(key: string) {
    this.#key = key;
  }

  async #query<T>(query: string, variables: Record<string, unknown> = {}): Promise<T> {
    const response = await fetch(API, {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: this.#key },
      body: JSON.stringify({ query, variables }),
    });
    const body = (await response.json()) as { data?: T; errors?: { message: string }[] };
    if (!response.ok || body.errors?.length || !body.data) {
      throw new Error(`Linear: ${body.errors?.map((e) => e.message).join("; ") ?? response.status}`);
    }
    return body.data;
  }

  async milestoneExists(epic: string): Promise<boolean> {
    const data = await this.#query<{ projects: { nodes: { projectMilestones: { nodes: { name: string }[] } }[] } }>(
      `query($project: String!) {
        projects(filter: { name: { eq: $project } }) {
          nodes { projectMilestones { nodes { name } } }
        }
      }`,
      { project: PROJECT },
    );
    return data.projects.nodes.some((p) => p.projectMilestones.nodes.some((m) => m.name === epic));
  }

  // The epic's frontier, as docs/agents/issue-tracker.md has it, narrowed to what an agent can
  // do alone: the `ready-for-agent` implementation tickets in Todo, unassigned, with every
  // blocker Done or Canceled. Oldest first.
  async frontier(epic: string): Promise<Ticket[]> {
    const data = await this.#query<{ issues: { nodes: IssueNode[] } }>(
      `query($project: String!, $epic: String!) {
        issues(first: 100, filter: {
          project: { name: { eq: $project } }
          projectMilestone: { name: { eq: $epic } }
          state: { name: { eq: "Todo" } }
          assignee: { null: true }
          and: [
            { labels: { some: { name: { eq: "ready-for-agent" } } } }
            { labels: { some: { name: { eq: "implementation" } } } }
          ]
        }) {
          nodes {
            id identifier title description createdAt
            inverseRelations { nodes { type issue { state { type } } } }
            comments { nodes { body user { name } } }
          }
        }
      }`,
      { project: PROJECT, epic },
    );
    return data.issues.nodes
      .filter((issue) =>
        issue.inverseRelations.nodes.every(
          (r) => r.type !== "blocks" || ["completed", "canceled"].includes(r.issue.state.type),
        ),
      )
      .sort((a, b) => a.createdAt.localeCompare(b.createdAt))
      .map((issue) => ({
        id: issue.id,
        identifier: issue.identifier,
        title: issue.title,
        description: issue.description ?? "",
        createdAt: issue.createdAt,
        comments: issue.comments.nodes.map((c) => ({ author: c.user?.name ?? "?", body: c.body })),
      }));
  }

  async #stateId(name: string): Promise<string> {
    if (!this.#states.size) {
      const data = await this.#query<{ workflowStates: { nodes: { id: string; name: string }[] } }>(
        `query($team: String!) {
          workflowStates(filter: { team: { name: { eq: $team } } }) { nodes { id name } }
        }`,
        { team: TEAM },
      );
      for (const state of data.workflowStates.nodes) this.#states.set(state.name, state.id);
    }
    const id = this.#states.get(name);
    if (!id) throw new Error(`Linear: the team ${TEAM} has no state ${name}`);
    return id;
  }

  async #update(id: string, input: Record<string, unknown>): Promise<void> {
    await this.#query(
      `mutation($id: String!, $input: IssueUpdateInput!) { issueUpdate(id: $id, input: $input) { success } }`,
      { id, input },
    );
  }

  async claim(ticket: Ticket): Promise<void> {
    const { viewer } = await this.#query<{ viewer: { id: string } }>(`{ viewer { id } }`);
    await this.#update(ticket.id, { assigneeId: viewer.id, stateId: await this.#stateId("In Progress") });
  }

  // Hands a ticket the run couldn't finish back to the frontier.
  async release(ticket: Ticket): Promise<void> {
    await this.#update(ticket.id, { assigneeId: null, stateId: await this.#stateId("Todo") });
  }

  // release, for a signal handler, which can't wait for a promise: Sandcastle's own handler
  // exits the process as soon as it runs. The state ids are known from the claim.
  releaseSync(ticket: Ticket): void {
    const body = JSON.stringify({
      query: `mutation($id: String!, $input: IssueUpdateInput!) { issueUpdate(id: $id, input: $input) { success } }`,
      variables: { id: ticket.id, input: { assigneeId: null, stateId: this.#states.get("Todo") } },
    });
    // The key goes in on stdin, so it isn't on curl's command line.
    const out = execFileSync("curl", ["-sS", "-X", "POST", API, "-H", "Content-Type: application/json", "-H", "@-", "-d", body], {
      input: `Authorization: ${this.#key}\n`,
      encoding: "utf8",
    });
    if (!out.includes('"success":true')) throw new Error(`Linear: ${out}`);
  }

  // The Linear documents the ticket and its comments link to, as the spec, which the agents
  // can't read from the sandbox.
  async documents(ticket: Ticket): Promise<Doc[]> {
    const text = [ticket.description, ...ticket.comments.map((c) => c.body)].join("\n");
    const slugs = new Set([...text.matchAll(/linear\.app\/[^/\s]+\/document\/[\w-]*?([0-9a-f]{12})\b/g)].map((m) => m[1]));
    const docs: Doc[] = [];
    for (const slug of slugs) {
      const { document } = await this.#query<{ document: Doc }>(
        `query($id: String!) { document(id: $id) { title content } }`,
        { id: slug },
      );
      docs.push(document);
    }
    return docs;
  }

  async setState(ticket: Ticket, state: "Done" | "In Review"): Promise<void> {
    await this.#update(ticket.id, { stateId: await this.#stateId(state) });
  }

  async setDescription(ticket: Ticket, description: string): Promise<void> {
    await this.#update(ticket.id, { description });
  }

  async comment(ticket: Ticket, body: string): Promise<void> {
    await this.#query(
      `mutation($input: CommentCreateInput!) { commentCreate(input: $input) { success } }`,
      { input: { issueId: ticket.id, body } },
    );
  }
}
