# Handoff: a remote machine for ticket work with Claude Remote Control

Where a cloud session left this, to continue locally. Delete this file once the work is done.

## The goal

Work camino's Linear tickets with Claude Code on a machine of your own, driven from the Claude app
through Remote Control (`claude remote-control`), as cheaply as possible. Time isn't a constraint.

## What was decided, and why

- **Not Claude's cloud sessions.** Their GitHub access goes through Anthropic's proxy, so:
  - It reaches only the repos attached to the session.
  - It blocks GraphQL, so `gh pr` and `gh issue` fail and only `gh api` REST calls work.
  - It rejects branch deletions and tag pushes.
  - Your own token doesn't get around this, and network secrets never apply to GitHub.
  - A relay of your own (an AWS proxy, say) would sidestep a safeguard, so it was dropped.
- **Your own machine instead**, with your own `GH_TOKEN`. `gh` gets the whole API there.
- **Where it runs:**
  - Fly.io first: a shared-cpu-4x machine with 8 GB, a volume, and started and stopped by hand.
  - Then the idea of a Raspberry Pi 4 with 8 GB, which costs only electricity. That's the
    current direction.
- **Cost levers, wherever it runs:**
  - Use a cheaper model for routine tickets: Sonnet, or Haiku for mechanical ones.
  - Use one fresh session per ticket.
  - Turn off unused connectors.
  - Write tight `ready-for-agent` tickets.
  - Skip subagents and workflows unless a ticket needs them.

## What exists

On branch `claude/festive-gates-ohxbqp`, in `deploy/fly/`. Nothing here has been built or
deployed yet.

- `Dockerfile`:
  - It's `.sandcastle/Containerfile`'s package list, plus `github-cli` and `tmux`, on
    `menci/archlinuxarm:base-devel`. That base image is multi-arch, with amd64 and arm64 checked.
  - It sets `CLAUDE_CONFIG_DIR=/data/claude`, `GH_CONFIG_DIR=/data/gh` and
    `CARGO_HOME=/data/cargo`. It also writes them to `/etc/caminito.env` for login shells.
- `entrypoint`:
  - As root, it gives `/data` to `agent`, then runs again as `agent` through `setpriv`.
  - It runs `gh auth setup-git`, and clones the repo to `/data/camino` when it's missing (needs
    `GH_TOKEN`).
  - It sets the git author from `GIT_NAME` and `GIT_EMAIL`.
  - It runs `claude remote-control` in tmux session `claude`, retrying every 30 seconds until
    you're logged in, and again whenever it exits.
- `fly.toml`:
  - App `caminito-claude`, region `gru`, volume `caminito_data` at `/data`, shared-cpu-4x with
    8 GB.
  - `GIT_NAME` and `GIT_EMAIL` are still placeholders.
- `README.md`: the Fly setup, the login, and day-to-day use.

## Running it on the Pi (nothing's in the repo for this yet)

1. Use Raspberry Pi OS Lite **64-bit**, booted from a USB 3 SSD, with active cooling and swap or
   zram.
2. Install Docker: `curl -fsSL https://get.docker.com | sh`.
3. Build the image and start the container:

   ```sh
   docker build -t caminito-claude deploy/fly
   docker run -d --name caminito --restart unless-stopped \
     -v caminito_data:/data \
     -e GH_TOKEN=github_pat_... -e GIT_NAME="Your Name" -e GIT_EMAIL=you@example.com \
     caminito-claude
   ```

4. Log in once:

   ```sh
   docker exec -it -u agent caminito bash -l
   cd /data/camino
   claude
   ```

   In Claude: run `/login`, accept the trust prompt, add Linear's MCP server, then `/exit`.

5. If linking runs out of memory, add `-e CARGO_BUILD_JOBS=2` to the `docker run`.

## Not verified

- **Linear's MCP server and API keys.** Unconfirmed whether it accepts a personal API key as
  `Authorization: Bearer lin_api_...`. The command:
  `claude mcp add --scope user --transport http linear https://mcp.linear.app/mcp --header ...`.
  The fallback is a Linear connector on claude.ai, which a logged-in Claude Code picks up.
- **Remote Control on a headless machine.** Unconfirmed that `claude remote-control` runs fine
  in a detached tmux without a person at the terminal.
- **Fly.** Unconfirmed whether `fly ssh console` shells get the image's environment; that's why
  `/etc/caminito.env` exists. Fly's prices and its memory limits per size weren't checked
  either; `fly platform vm-sizes` lists the sizes.
- **Build time on the Pi.** A guess: an hour or more for a first full build.

## Next steps

1. Build the image on the Pi and fix whatever the first build breaks.
2. Log in, add Linear, and check that the session shows up in the Claude app.
3. Work one small ticket end to end: read it from Linear, branch, run `tools/check`, push, and
   open a PR with `gh pr create`.
4. Offered and not done yet:
   - Add a `compose.yaml` so it's `docker compose up -d`.
   - Add a Pi section to the README.
   - Rename `deploy/fly/` to `deploy/remote/`, since it's not only Fly's.
   - Add a line to `CLAUDE.md` for cloud sessions: use `gh api` REST calls, not `gh pr` or
     `gh issue`.
5. Delete the GitHub token from the earlier "option B" attempt if one was made. Make a
   fine-grained token for `JPabloLassala/camino` only, with these permissions:
   - Contents, Pull requests and Issues: Read and write.
   - Actions: Read-only.
