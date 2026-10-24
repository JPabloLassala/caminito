# Caminito's Fly.io machine

A Fly.io machine that works Caminito's tickets with Claude Code, driven from the Claude app
through Remote Control. It is the sandbox of `.sandcastle/Containerfile` with `gh` and `tmux`. Its
state lives on a volume at `/data`: Claude's and gh's logins, cargo's cache and the checkout with
its `target/`. A restart or a redeploy keeps all of it.

The machine has no HTTP service, so Fly never starts or stops it on its own. Start it when you
work tickets and stop it after: a stopped machine costs only its volume.

## Set it up

Install [flyctl](https://fly.io/docs/flyctl/install/) and log in with `fly auth login`. Then, from
this directory:

1. Set the commit author in `fly.toml`: `GIT_NAME` and `GIT_EMAIL`. Pick `primary_region`
   (`fly platform regions` lists them).
2. Create the app and the volume, in the same region:

   ```sh
   fly apps create caminito-claude
   fly volumes create caminito_data --region gru --size 30
   ```

3. Give the machine a GitHub token. Make a fine-grained one at
   <https://github.com/settings/personal-access-tokens>, for `JPabloLassala/camino` only, with
   Contents, Pull requests and Issues as Read and write, and Actions as Read-only.

   ```sh
   fly secrets set GH_TOKEN=github_pat_...
   ```

4. Deploy. The first build takes a while, for Arch's packages:

   ```sh
   fly deploy
   ```

   On its first boot the machine clones the repo to `/data/camino`.

## Log in

Once, over SSH, as the `agent` user:

```sh
fly ssh console -u agent
cd /data/camino
claude
```

Log in with your Claude account (`/login`: open the URL it prints, paste the code back) and
accept the folder's trust prompt. To give Claude the Linear tools, add Linear's MCP server with a
personal API key from Linear's Settings > Security & access:

```sh
claude mcp add --scope user --transport http linear https://mcp.linear.app/mcp \
  --header "Authorization: Bearer lin_api_..."
```

Leave Claude with `/exit` and SSH with `exit`. Within 30 seconds the machine starts Remote
Control, and the session shows up in the Claude app.

## Use it

```sh
fly machine start    # before working tickets
fly machine stop     # after
```

When the machine starts, it runs `claude remote-control` in `/data/camino` in a tmux session, and
runs it again if it exits. The session shows up in the Claude app (claude.ai/code and the mobile app). To look at it from a
terminal, run `fly ssh console -u agent` and then `tmux attach -t claude`.

`gh` has the whole API in Claude's session, `gh pr` and `gh issue` included: it's your own token,
without the cloud sessions' proxy. An SSH shell doesn't have `GH_TOKEN`, so `gh` isn't logged in
there.

## Change it

Change the `Dockerfile` and run `fly deploy` again. The volume isn't touched. Keep the package
list in step with `.sandcastle/Containerfile`'s.
