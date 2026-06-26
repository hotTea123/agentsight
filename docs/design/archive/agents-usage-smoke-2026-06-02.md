# Agent CLI usage with AgentSight

> Archived smoke-test note from 2026-06-02. This may be stale; prefer the
> current source code and user-facing docs in the repository root and
> `docs/usage.md`.

This document records the agent commands that were tested on this machine on
2026-06-02. It is intentionally conservative: if a command only passed a syntax
or startup smoke, it is marked that way.

AgentSight does not require a registered agent allowlist. There is no
`known_agent_cli --capture-cli-output` path: stdout/stderr from the wrapped
agent are left on the terminal and are not written to SQLite. AgentSight records
OS-side process/file/network facts, then derives model/token data only from
network request/response/telemetry that can be parsed or from explicit
agent-native local session logs such as `~/.claude` and `~/.codex`.
Use `agentsight report prompts --json` to inspect LLM request/response bodies that
were stored in SQLite from parsed network traffic or SQL adapters.

Use the debug binary shown below from the repository root, or replace
`./collector/target/debug/agentsight` with `agentsight` after installation.

## Common checks

```bash
sudo -n true
./collector/target/debug/agentsight --help
./collector/target/debug/agentsight discover
```

Tested local versions:

| Agent | Version tested | Local command |
| --- | --- | --- |
| Claude Code | `2.1.160` | `claude` |
| Codex CLI | `codex-cli 0.136.0` | `codex` |
| OpenCode | `1.15.13` | `opencode` |
| Gemini CLI | `0.28.1` | `gemini` |
| OpenClaw | `2026.5.28` container | `ghcr.io/openclaw/openclaw:latest` |

## Claude Code

Direct non-interactive command:

```bash
claude -p 'Reply with exactly: agentsight-smoke' \
  --output-format json \
  --tools '' \
  --permission-mode dontAsk
```

AgentSight capture:

```bash
sudo -n env PATH="$PATH" HOME="$HOME" \
  ./collector/target/debug/agentsight record \
  --no-server \
  --db /tmp/agentsight-claude.db \
  --adapter auto \
  -o /tmp/agentsight-claude.log \
  -- claude -p 'Reply with exactly: agentsight-smoke' --output-format json
```

Summary:

```bash
./collector/target/debug/agentsight report --db /tmp/agentsight-claude.db
./collector/target/debug/agentsight report prompts --db /tmp/agentsight-claude.db --json
./collector/target/debug/agentsight report --local
```

Evidence sources:

- OS process/file/network facts from AgentSight `record`.
- Token/model data from Anthropic response usage, Claude Code telemetry request
  payloads, or Claude-native local session logs under `~/.claude/projects`.
- Not from Claude Code terminal stdout/stderr.

Test result on this machine:

- `claude --version` and `claude --help` passed.
- The direct prompt command reached Claude Code but returned a Claude account
  `five_hour` rate-limit error (`429`), so the command shape is verified but a
  full live reply was not available in this account window.

## Codex CLI

Direct non-interactive command:

```bash
tmp=$(mktemp -d)
codex exec \
  --cd "$tmp" \
  --skip-git-repo-check \
  --sandbox read-only \
  'Reply with exactly: agentsight-smoke'
rm -rf "$tmp"
```

AgentSight capture:

```bash
tmp=$(mktemp -d)
sudo -n env PATH="$PATH" HOME="$HOME" \
  ./collector/target/debug/agentsight record \
  --no-server \
  --db /tmp/agentsight-codex.db \
  --adapter auto \
  -o /tmp/agentsight-codex.log \
  -- codex exec \
    --cd "$tmp" \
    --skip-git-repo-check \
    --sandbox read-only \
    'Reply with exactly: agentsight-smoke'
rm -rf "$tmp"
```

Local Codex summary:

```bash
./collector/target/debug/agentsight report --local
```

Evidence sources:

- OS process/file/network facts from AgentSight `record`.
- Token/model/tool data from Codex-native JSONL session logs under
  `~/.codex/sessions`.
- Current SQLite SQL adapters do not include a Codex-specific projection.
  In the tested run, the AgentSight SQLite summary correctly showed OS facts
  without token claims; `report --local` showed Codex model, tokens, and
  tool calls from `.codex/sessions`.

Test result on this machine:

- `codex --version`, `codex --help`, and `codex exec --help` passed.
- Direct Codex prompt passed and printed `agentsight-smoke`.
- AgentSight-wrapped Codex prompt passed and printed `agentsight-smoke`.
- `agentsight report --local` returned a `codex session` with model/token
  and tool-call summary.
- Integration test added: `cargo test --test export_snapshot_test local_summary_reads_codex_session_jsonl`.

## OpenCode

Direct non-interactive command:

```bash
tmp=$(mktemp -d)
opencode run --dir "$tmp" 'Reply with exactly: agentsight-smoke'
opencode stats --days 1 --models
rm -rf "$tmp"
```

Raw event mode:

```bash
tmp=$(mktemp -d)
opencode run --dir "$tmp" --format json 'Reply with exactly: agentsight-smoke'
rm -rf "$tmp"
```

AgentSight capture:

```bash
tmp=$(mktemp -d)
sudo -n env PATH="$PATH" HOME="$HOME" \
  ./collector/target/debug/agentsight record \
  --no-server \
  --db /tmp/agentsight-opencode.db \
  --no-adapters \
  -o /tmp/agentsight-opencode.log \
  -- opencode run --dir "$tmp" 'Reply with exactly: agentsight-smoke'
rm -rf "$tmp"
```

Evidence sources:

- OS process/file/network facts from AgentSight `record`.
- OpenCode token/cost/session data from OpenCode itself, for example
  `opencode stats --days 1 --models` and its local database/log files.
- Not from terminal stdout/stderr.

Test result on this machine:

- `opencode --version`, `opencode --help`, `opencode run --help`,
  `opencode stats --help`, and `opencode providers list` passed.
- Direct `opencode run` exited 0. In this environment it printed a model header
  rather than a final plain-text answer, so final text was not asserted.
- `opencode stats --days 1 --models` showed sessions, model token usage, and
  tool usage.
- AgentSight-wrapped OpenCode run exited 0 and the SQLite summary showed
  subprocesses plus access to OpenCode log/database files.

## Gemini CLI

Direct non-interactive command:

```bash
gemini -p 'Reply with exactly: agentsight-smoke' --output-format json
```

AgentSight capture:

```bash
sudo -n env PATH="$PATH" HOME="$HOME" \
  ./collector/target/debug/agentsight record \
  --no-server \
  --db /tmp/agentsight-gemini.db \
  --adapter auto \
  -o /tmp/agentsight-gemini.log \
  -- gemini -p 'Reply with exactly: agentsight-smoke' --output-format json
```

Evidence sources:

- OS process/file/network facts from AgentSight `record`.
- Gemini token data only when AgentSight parses network response usage such as
  Gemini `usageMetadata`.
- Gemini's own `--output-format json` prints token stats to the terminal, but
  AgentSight does not ingest that stdout as SQLite evidence.

Test result on this machine:

- `gemini --version`, `gemini --help`, and direct prompt passed.
- Direct Gemini JSON output returned `agentsight-smoke` and model token stats.
- AgentSight-wrapped Gemini prompt passed and returned `agentsight-smoke`.
- In this specific AgentSight run, SQLite summary had no token rows because the
  network stream did not expose parseable usage to AgentSight; this is expected
  and should not be replaced by stdout scraping.

## OpenClaw

Pull and start a local gateway container:

```bash
docker pull ghcr.io/openclaw/openclaw:latest

docker run -d --name openclaw \
  -p 127.0.0.1:19001:19001 \
  ghcr.io/openclaw/openclaw:latest \
  node openclaw.mjs gateway run \
    --allow-unconfigured \
    --auth none \
    --bind loopback \
    --port 19001 \
    --force \
    --raw-stream \
    --raw-stream-path /tmp/openclaw-raw.jsonl
```

AgentSight container attach:

```bash
sudo -n env PATH="$PATH" HOME="$HOME" \
  ./collector/target/debug/agentsight record \
  -c node \
  --binary-path docker://openclaw \
  --db /tmp/agentsight-openclaw.db \
  --adapter openclaw \
  -o /tmp/agentsight-openclaw.log \
  --server-port 7396
```

Provider-backed agent turn, after configuring provider credentials:

```bash
docker exec openclaw \
  node openclaw.mjs agent \
    --message 'Reply with exactly: agentsight-smoke' \
    --json
```

Evidence sources:

- OS process/file/network facts from AgentSight attached to the container's
  Node process.
- Token/model data from provider network usage when captured and parsed, or
  from OpenClaw's own raw stream/session data.

Test result on this machine:

- Docker daemon was available.
- `docker pull ghcr.io/openclaw/openclaw:latest` passed.
- Container CLI help, `gateway run --help`, and `agent --help` passed.
- Gateway start smoke passed; logs showed `http server listening` and `ready`.
- AgentSight `record -c node --binary-path docker://<container>` resolved the
  container init process to the SSL-embedding Node host PID and attached. The
  smoke was stopped with `timeout`, so provider inference was not run.
- `OPENAI_API_KEY` was not set in this shell, so a provider-backed OpenClaw
  agent turn was not executed.

## What not to do

Do not reintroduce a generic `--capture-cli-output` path for agent stdout/stderr.
Terminal output is useful for the human operator, but it is not reliable
evidence of state changes. If an agent writes structured local logs, add an
explicit log reader for that fixed location and schema. If tokens are present in
network responses or telemetry requests, parse those network payloads. Otherwise
leave token fields absent rather than inventing `0 tokens` or scraping terminal
text.
