# AgentSight Product Scope: Scenario-Driven, Not Feature-Driven

This note is a product design checkpoint for AgentSight. The main concern is
that in the agent era, some software code becomes dead weight. If a user's
agent can already solve a domain task with natural language and tools, then
AgentSight should not rebuild that task as a fixed workflow.

AgentSight should become a product only where software adds durable value that
the acting agent cannot safely provide by itself.

The short version:

> Let agents do the work. Let AgentSight make the work accountable.

Accountability does not only mean audit after the fact. The broader product
problem is delegation: users want agents to act on real systems, but they need
confidence, control, and recovery when that delegation has side effects.

## Product Principle

AgentSight should not automate the user's task. The user's agent does that.

AgentSight should observe, attribute, explain, and preserve evidence.

Current CLI note: the implemented capture entrypoint is `agentsight record`,
with `agentsight top` as the default live view and `agentsight report` for saved
run inspection. Examples below use those command family names; options such as
`--policy` or `--label` are future product proposals unless they appear in
`agentsight --help`.

Use this rule when deciding whether to write code:

- If the feature decides what to do in a domain, prefer the agent.
- If the feature proves what happened, preserves evidence, correlates events, or
  enforces a trust boundary, prefer AgentSight.

This keeps AgentSight from becoming a pile of brittle, product-specific
automation.

## Scenario Map

The product should be organized around user scenarios, not telemetry categories.

| Scenario | User | When it happens | Product value |
| --- | --- | --- | --- |
| Delegation confidence | Individual/team agent user | Before increasing agent autonomy | "Can I let it keep working without watching every step?" |
| Permission tuning | Individual/team agent user | While configuring approvals or policy | "Which actions should be auto-approved, confirmed, or blocked?" |
| Run receipt | Individual agent user | After a normal agent session | "What did it actually do?" |
| Recovery context | Developer/user | After local state is changed or damaged | "What do I need to undo or inspect?" |
| Review and acceptance | Skill/MCP/tool reviewer | Before trusting a new agent tool | "Does it really behave as claimed?" |
| Incident forensics | Developer/security/user | After something broke or looked suspicious | "What caused this and what was touched?" |
| PR due diligence | Code reviewer/team lead | When reviewing agent-generated code | "Can I trust the process behind this diff?" |
| Behavioral regression testing | Agent/skill/product builder | During CI or release testing | "Did this new version expand its behavior?" |
| Live airlock | High-risk operator/team | During dangerous agent actions | "Should this action be allowed now?" |
| Cost/resource accounting | Team/platform owner | Across many runs or users | "Where did the agent spend money and resources?" |

These are not all equally important. The first three are the strongest product
directions because they rely on independent evidence, not domain automation.

Audit is only one of these scenarios. If the product is framed only as audit,
it sounds useful mainly after something bad happens. The larger opportunity is
to make agent delegation more usable before, during, and after a run.

## Scenario 1: Delegation Confidence

User:

- an individual developer, team lead, or platform owner deciding how much
  autonomy to give local or CI agents

Situation:

- the agent is useful, but the user is still watching every step because they
  do not know whether it will stay inside expected boundaries
- the user wants fewer interruptions without blindly allowing everything

User question:

> Can I let this agent keep working without watching every step?

Why the user's agent is not enough:

- the acting agent can promise it will stay inside bounds, but cannot prove its
  historical behavior independently
- permission prompts often show a command or short summary, not the wider
  system effect pattern
- a user cannot tune autonomy well without knowing which actions are actually
  common, safe, rare, or risky

AgentSight output:

- historical behavior profile by agent/tool/repo
- common commands and paths
- writes inside vs outside workspace
- network destinations
- actions that usually succeed without side effects
- actions that frequently touch sensitive paths or require review

Product shape:

```bash
agentsight report --autonomy
agentsight policy suggest
```

This turns evidence into a practical decision: what can be auto-approved, what
should ask, and what should be denied.

## Scenario 2: Permission Tuning

User:

- a developer or team configuring Codex/Claude/Cursor/Gemini approvals, MCP
  allowlists, or internal agent policy

Situation:

- too many confirmations make the agent annoying
- too few confirmations make it unsafe
- generic policy is either too permissive or too restrictive

User question:

> Which actions should this agent be allowed to do automatically?

AgentSight output:

- policy suggestions from observed behavior
- "safe so far" patterns, such as project-local test commands
- "needs confirmation" patterns, such as writes outside workspace
- "deny" patterns, such as secret reads plus unknown network egress
- evidence backing each recommendation

Product shape:

```bash
agentsight policy suggest --from last-20-runs
agentsight record --policy suggested.yaml -- claude
```

This is not just audit. It is a feedback loop that makes agent autonomy more
usable.

## Scenario 3: Run Receipt

User:

- an individual developer or power user running Claude Code, Codex, Gemini CLI,
  or another local agent

Situation:

- the user asks the agent to edit code, run commands, clean files, install
  dependencies, analyze a repo, or operate the local machine

User question:

> I used an agent normally. What did it actually do to my system?

Why the user's agent is not enough:

- the acting agent can summarize what it believes it did, but it is not an
  independent source of truth
- shell history does not capture all child processes, file operations, network
  destinations, or LLM calls
- git diff only shows final tracked file changes, not reads, secrets access,
  failed attempts, background processes, or untracked side effects

AgentSight output:

- session duration
- model calls and token usage
- commands executed
- process tree
- files read, written, renamed, deleted, or created
- network destinations
- risky or unexpected actions
- actions not attributable to an explicit tool call

Product shape:

```bash
agentsight record -- claude
agentsight report
```

This is the smallest useful product. It should work even if the user never
opens a full timeline UI.

## Scenario 4: Recovery Context

User:

- a developer or user trying to clean up after an agent changed local state

Situation:

- the repo is broken
- files were deleted or generated
- dependencies were installed
- a shell profile, config file, cache, or service state may have changed

User question:

> What do I need to undo or inspect to get back to a good state?

Why git diff is not enough:

- it misses untracked files
- it misses writes outside the repo
- it misses package installs and cache changes
- it misses background processes and network side effects
- it does not show the command chain that produced the state

AgentSight output:

- changed path inventory
- destructive operation list
- process lineage
- commands that modified local state
- possible recovery context for a human or another agent

Product shape:

```bash
agentsight report --recovery
agentsight report export -o recovery.json
```

This makes agent side effects recoverable, not merely auditable.

## Scenario 5: Review And Acceptance

User:

- someone evaluating a new skill, MCP server, agent plugin, workflow, or local
  automation before trusting it

Situation:

- a tool claims it is read-only, project-local, reversible, or safe
- the reviewer wants evidence before installing, publishing, approving, or
  recommending it

User question:

> Does this agent tool really behave the way it claims?

Why this is different from a normal run receipt:

- the user is not just curious about one session
- the user is deciding whether a tool should be trusted by other people
- the output needs to be repeatable and shareable

AgentSight output:

- declared behavior vs observed behavior
- whether the run was read-only
- whether writes stayed inside allowed paths
- whether network calls matched expectations
- whether secrets or credentials were accessed
- exact commands, processes, and file effects
- static report that can be attached to a README, issue, PR, or marketplace page

Product shape:

```bash
agentsight verify -- claude
agentsight report --format html
```

This is a strong scenario for the cleanup skill example. The value is not that
AgentSight knows how to clean disks. The value is that it can verify the cleanup
agent only scanned, recommended, and deleted what the user confirmed.

## Scenario 6: Incident Forensics

User:

- a developer, security engineer, or advanced user investigating something that
  went wrong after an agent session

Situation:

- files disappeared
- a config changed
- a secret may have been read
- an unexpected external request happened
- a repo is now broken
- a local machine or CI worker is in a strange state

User question:

> Something went wrong. Which agent action caused it, and what exactly was
> touched?

Why this is a different product scenario:

- the user is no longer in a calm "review" mode
- the session may be over
- the acting agent may have crashed, hallucinated, or lost context
- the user needs evidence, not a plausible explanation

AgentSight output:

- incident timeline
- suspicious actions ranked by risk
- destructive file operations
- writes outside the working directory
- commands that modified system state
- network requests near the suspicious time window
- process lineage from LLM/tool call to OS effect
- enough evidence for another agent or human to propose recovery

Product shape:

```bash
agentsight report --incident
agentsight report export -o incident.json
```

This scenario is not about preventing all failures. It is about making failures
reconstructable.

## Scenario 7: PR Due Diligence

User:

- a reviewer looking at code produced by an agent
- a team lead deciding whether agent-generated PRs are trustworthy

Situation:

- a PR contains code changes, but the reviewer did not observe the agent run
- the final diff is not enough to know whether the process was safe

User question:

> What happened during the agent run that produced this PR?

Why git diff is not enough:

- it does not show commands run during generation
- it does not show tests that failed before passing
- it does not show downloads, installers, package managers, or external scripts
- it does not show whether private files were read
- it does not show untracked generated artifacts

AgentSight output:

- files changed
- commands and tests run
- dependency or package manager activity
- external network destinations
- sensitive path access
- failed and repeated attempts
- final "review risk" summary attached to the PR

Product shape:

```bash
agentsight record -- codex
agentsight report --for-pr
```

This is a social trust scenario. The person who needs AgentSight may not be the
person who ran the agent.

## Scenario 8: Behavioral Regression Testing

User:

- an agent developer, skill author, MCP author, or AI product team

Situation:

- a new prompt, model, tool version, skill, or MCP server release changes agent
  behavior
- the team wants to know if the new version has a broader or riskier system
  footprint than the old version

User question:

> Did this new agent version change what it does to the system?

Why this is genuinely different:

- the goal is not one user's trust in one run
- the goal is repeated comparison across versions
- the artifact is a behavior diff, not a one-time receipt

AgentSight output:

- old vs new process tree
- old vs new file effects
- old vs new network destinations
- old vs new token and runtime cost
- newly touched sensitive paths
- newly introduced destructive operations
- behavior expansion score

Product shape:

```bash
agentsight record --label old -- ./agent-task.sh
agentsight record --label new -- ./agent-task.sh
agentsight diff old new
```

This may become valuable for CI because it catches changes that ordinary output
evals miss. A model can produce the same final answer while touching very
different system resources.

## Scenario 9: Live Airlock

User:

- an enterprise user, platform team, or security-sensitive operator running
  agents with meaningful local or production access

Situation:

- after-the-fact reporting is not enough
- some actions should be stopped or require human confirmation before they
  happen

User question:

> Should this agent action be allowed right now?

Why this should not be the first product:

- live blocking is harder than observation
- false positives can break normal agent workflows
- general-purpose "agent firewall" rules become vague quickly
- enforcement should be built only after the evidence/reporting layer is solid

Where it could be valuable:

- deny destructive writes outside a workspace
- require confirmation before accessing credential paths
- require confirmation before sending data to unknown domains
- block shell profile edits
- block package install scripts in locked-down environments

Product shape:

```bash
agentsight record --policy workspace-only -- claude
```

This is a later-stage scenario. It has real value, but only if the policy
surface stays small and concrete.

## Scenario 10: Cost And Resource Accounting

User:

- team owner, platform engineer, or manager of shared agent infrastructure

Situation:

- many agents or many users run tasks on shared machines or CI workers
- someone needs to understand token spend, runtime, CPU, memory, process count,
  and wasted work

User question:

> Which agent sessions consumed money and resources, and why?

Why this is not just another dashboard:

- the useful unit is a session or task, not a generic host metric
- cost needs to connect to the work performed and system effects produced
- runaway reasoning loops and repeated failed tool calls are agent-specific
  waste patterns

AgentSight output:

- token spend by session, model, user, or task
- long-running or looping sessions
- repeated failed commands
- high process count or memory usage
- resource cost linked to visible work

This is useful for teams, but it is less urgent than run receipts, review, and
incident forensics for the first product.

## What User Agents Can Still Do

AgentSight should not replace the acting agent's reasoning.

| Question | Better handled by |
| --- | --- |
| Which Bilibili videos should I delete? | User's agent |
| Did the agent delete only those videos? | AgentSight |
| Which old project directories look unused? | User's agent |
| Did the agent write outside the selected directory? | AgentSight |
| Explain whether this cache is safe to clean. | User's agent |
| Show every file operation performed during cleanup. | AgentSight |
| Summarize this evidence in natural language. | Agent, using AgentSight evidence |
| Preserve an independent audit record. | AgentSight |
| Decide how to fix a broken repo. | User's agent or human |
| Show which command broke the repo. | AgentSight |

The product should produce structured evidence. Agents can then consume that
evidence to explain, fix, or decide.

## Report Shape

The report should be structured around user questions, not telemetry categories.

Recommended sections:

1. **Summary**

   - what agent ran
   - how long it ran
   - how many model calls happened
   - total token usage
   - total commands and file effects
   - top risks

2. **What Changed**

   - files written
   - files deleted or moved
   - directories created
   - package or dependency changes
   - generated artifacts

3. **What Ran**

   - process tree
   - commands
   - shell scripts
   - package managers
   - background processes

4. **Where Data Went**

   - LLM providers
   - other network destinations
   - suspicious egress

5. **Risk Flags**

   - green: normal project-local behavior
   - yellow: user should review
   - red: destructive, sensitive, or unexpected

6. **Evidence**

   - raw events
   - exact paths
   - command arguments
   - timestamps
   - process lineage

## Development Direction

The next product work should be narrow:

1. Build a useful run receipt from existing SQLite data.
2. Improve attribution from LLM/tool calls to process and file effects.
3. Add simple risk flags for destructive, sensitive, or unexpected behavior.
4. Make the report static and shareable.
5. Add a verification/report mode for skill and MCP review.
6. Add behavior diff only after the single-run report is useful.
7. Keep the full dashboard as an inspection tool, not the primary product.

Avoid broad feature expansion until the report is good enough that a user can
run an agent, finish a task, and immediately understand what happened.

## Non-Goals For Now

- Do not build domain-specific cleanup logic into AgentSight.
- Do not build a general agent IDE.
- Do not build a full enterprise policy engine first.
- Do not require users to understand raw eBPF events.
- Do not make the main value depend on a live dashboard.
- Do not use an LLM summary as a substitute for observable evidence.
- Do not build a generic workflow builder.
- Do not become another prompt/latency/token dashboard.

## Feature Kill Test

Before adding product code, ask:

1. Does this provide independent evidence?
2. Does this work across multiple agent tasks and domains?
3. Would a user distrust the acting agent's self-report here?
4. Does git diff, shell history, or normal logs fail to answer the question?
5. Does this reduce risk, improve accountability, or make recovery easier?
6. Is this better as product code than as an agent prompt over existing
   evidence?

If the answer is mostly no, the feature is probably dead weight.

## The Product Bet

Agent software will become more fluid and disposable. Users will increasingly
ask agents to do tasks that used to require fixed applications.

That makes many traditional software features less valuable.

But it makes independent evidence more valuable.

The product bet is:

> As agents replace task-specific tools, users and teams will need a small,
> trusted layer that records what those agents actually did.

That layer is where AgentSight can become a product.
