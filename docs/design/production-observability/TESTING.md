# Production Observability Testing

Current tests should exercise the direct view path:

- raw runner `Event` values enter `MaterializedView::ingest_event`
- snapshots expose typed rows
- SQLite loads rows back into `MaterializedView`
- web API serves the shared live view
- frontend consumes `/api/v1/snapshot`

Do not add AgentSight JSONL replay fixtures. Agent-native local session logs
such as Claude/Codex `.jsonl` files remain valid input sources for local
session summaries, but they are not AgentSight capture persistence.
