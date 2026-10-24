// What the host side of a Sandcastle run asks of Linear, through its GraphQL API. The sandbox
// never sees the API key: the host claims and resolves the tickets, and the agents only code.

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
