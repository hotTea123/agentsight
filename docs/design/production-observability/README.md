# Production Observability

The production path is view-first:

```text
runner Event -> analyzers -> MaterializedView
                         |-> SQLite row sink
                         |-> OTel row sink
                         |-> shared Web API snapshot
                         |-> CLI/TUI queries
```

AgentSight no longer writes or replays its own JSONL capture logs. The durable
store is SQLite materialized rows; the live web server reads the shared in-memory
view.

The API contract is the snapshot and focused row endpoints:

```text
GET /api/v1/snapshot?audit_limit=
GET /api/v1/summary
GET /api/v1/token-summary?group_by=model|provider|comm|pid
GET /api/v1/audit-events?audit_limit=
GET /api/v1/process-nodes
GET /api/v1/sessions
GET /api/v1/agents
```

The frontend consumes `/api/v1/snapshot` as the single contract. Process tree UI
is built from `process_nodes` plus timestamp-matched `audit_events`; resource UI
uses `resource_samples`.
