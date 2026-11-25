# stapel in Zed through ACP

## Description

The person works on tickets and features in Zed's agent panel. `stapel acp` is the agent that Zed starts. It
stands between Zed and Claude Code (the adapter `claude-code-acp`) on the Agent Client Protocol and passes the
messages both ways. It adds what stapel knows: the next action, the Plan as a live plan, slash commands,
confirmations and questions as buttons, and the guard on every tool call. The builder and the reviewers run as
sessions that stapel starts, so their work shows live. A feature has one overview session over its tickets.
Inputs: the chat of 2026-11-25 (variant 2 and path B), `docs/PLAN.md` items 5.3 (MCP) and the LSP server.

## Outcome

The person carries tickets and features from start to merge inside Zed, answers each process point with a
button, and sees every agent's work as it happens, on their Claude Code login.

## Done when

- A ticket runs from Frame to close in a Zed session through `stapel acp`, with every confirmation and
  question answered in the panel and no terminal dialog.
- The reads, edits, test runs and commit of a builder show in the panel while the builder works.
- A feature with two parallel tickets is followed and answered from one overview session.
- `stapel` makes no model call of its own; the model calls go through the agent's own login.
- Work in the terminal without Zed passes its existing tests.

## Boundaries

- Out: `stapel` as an agent that calls a model itself (variant 3).
- Out: an editor other than Zed; the protocol may allow it, but no other editor is tested.
- Out: the MCP server (plan item 5.3) and the LSP server; this feature uses them and does not change them.
- Must not change: the terminal path with the PreToolUse guard and the `stapel ok` dialog of Claude Code.
- Must not change: the facts on disk that `stapel next` reads; the panel writes the same records as the commands.

## Requirements

R1: The person can pick `stapel` as an agent in Zed and work with Claude Code through it on the Claude Code login; shown by a prompt in Zed that reaches Claude Code and its answer, tool calls and diffs that come back.
R2: A session shows the open ticket and its next action at its start; shown by a first message that names the ticket and the action that `stapel next` prints.
R3: The stapel commands of the process are slash commands in the panel; shown by `/next`, `/new`, `/check`, `/page` and `/close` listed, each running its command.
R4: The person answers a confirmation in the panel after the diff of the changed sections; shown by a confirm that writes the same record as `stapel ok`, and a reject that writes none.
R5: The person answers a question of a process point (Frame questions, `ask-human`, `intent_gap`) with buttons, the recommendation marked; shown by the answer in Decisions or in the triage of the finding.
R6: The Plan steps of the ticket show as a live plan in the panel, with each status taken from stapel facts and not from the agent; shown by a step that turns done only after its commit and check.
R7: Every tool call of an agent in the session is checked by the rules of the PreToolUse guard before it runs; shown by a cargo call outside `stapel cargo` refused in a Zed session, with its reason in the panel.
R8: The builder and each reviewer run as sessions that stapel starts, each with the instructions and model of its role, and their actions show live inside the session that started them; shown by a builder's reads, edits, test runs and commit visible as they happen.
R9: Each session that stapel starts is recorded in the token journal of the ticket with its role; shown by `stapel tokens KEY` listing the builder and reviewer sessions of a Zed run.
R10: The external reviewer can be any ACP agent that `stapel.toml` names, Claude Code on opus by default; shown by a review round run with a second agent.
R11: A feature has one overview session that lists its tickets with a status taken from stapel facts; shown by statuses that match `stapel next` in each worktree.
R12: Parallel tickets of a feature each run in their own session, and the questions and confirmations of any ticket reach the overview session; shown by an `ask-human` of one ticket answered from the overview.
R13: Work in the terminal without Zed stays complete; shown by the existing suite passing and the PreToolUse guard unchanged.

## Constraints

- No model calls billed by stapel: the person works on a subscription; every model call goes through the
  agent's own login (the person, 2026-11-25: tokens or subscription was the question).
- The terminal path stays complete beside the proxy (the person, 2026-11-25).
- The protocol still changes: the proxy follows a pinned version of ACP and of `claude-code-acp`.

## Decisions

- 2026-11-25: Decision: the person chose variant 2, `stapel` as an ACP proxy between Zed and Claude Code.
- 2026-11-25: Decision: the person chose path B: stapel starts the builder and reviewer sessions itself.
- 2026-11-25: Decision: the person chose that the draft covers the proxy, path B and the feature overview.
- 2026-11-25: Decision: the person chose that the terminal path stays complete.
- 2026-11-25: Decision: the person chose that the external reviewer can be any ACP agent.
- 2026-11-25: Decision: the person said the plan does not change now; no plan item holds this feature yet.
- 2026-11-25: Assumption: `claude-code-acp` runs on the person's Claude Code login with a subscription, as it does in Zed today.
- 2026-11-25: Open question: does `claude-code-acp` pass the inner steps of a `Task` call, or only its result?
- 2026-11-25: Open question: can Zed show several sessions of one agent at once and switch between them?
- 2026-11-25: Open question: how does Zed render live nested content inside one tool call?
- 2026-11-25: Open question: do Anthropic's terms allow a tool to start Claude Code sessions through the adapter on a subscription login?
- 2026-11-25: Open question: which plan item takes this feature, and after which items?

## Breakdown

- 1. Pass-through proxy with the start message: covers R1, R2, R13 (the proxy leaves the terminal path as it is); after none; done when a test ACP client sends a prompt through `stapel acp` to a fake agent, gets the answer back and first gets the next action of the open ticket.
- 2. Slash commands: covers R3; after 1; done when a test client lists the five commands and `/next` returns the action of `stapel next`.
- 3. Confirmations and questions as buttons: covers R4, R5; after 1; done when a test client answers a permission request with confirm and the confirmation record equals that of `stapel ok`.
- 4. Live plan: covers R6; after 1; done when a test client receives the plan of a fixture ticket and a step turns done after its commit and check.
- 5. Guard in the proxy: covers R7, R13 (the guard rules stay shared with the PreToolUse guard); after 1; done when a fake agent's cargo call outside `stapel cargo` is refused with its reason.
- 6. Builder sessions: covers R8 (the builder), R9 (the builder); after 1; done when a fake builder session's tool calls reach the test client as live nested content and its tokens land in the journal with the role `builder`.
- 7. Reviewer sessions on any agent: covers R8 (the reviewers), R9 (the reviewers), R10; after 6, because it reuses the session start of ticket 6; done when a review round runs with two fake agents named in `stapel.toml` and their findings are imported.
- 8. Feature overview: covers R11; after 4, because it reuses the plan of ticket 4; done when a test client receives the tickets of a fixture feature with the statuses of `stapel next`.
- 9. Parallel tickets and questions in the overview: covers R12; after 3 and 8, because it routes the buttons of ticket 3 to the overview of ticket 8; done when an `ask-human` of one fixture ticket is answered from the overview session.
