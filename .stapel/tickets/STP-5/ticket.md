# STP-5 — Measured token usage of subagents

## Description

Source: the close of STP-4 (Summary, "measurement caveat"); `docs/PHASES.md`, phase 0, STP-5. First ticket of
the spec process of `CLAUDE.md` after STP-4 (pilot).

### Frame

**Problem.** `stapel tokens import` takes the usage of each assistant message from its last transcript
line. In subagent transcripts most messages never get a final line: of 77 messages in the 14 subagent files
of STP-4, 8 have one (`stop_reason` set); the rest carry the usage of the first streamed chunk, so output is
4 or 5 tokens where the report was thousands. Input and cache counts of those lines look right; output and
thinking are lost. The STP-4 journal therefore shows 3.6k output tokens for all subagents.

Facts found while framing (2026-10-05, this machine):

- The main transcript's `cost-state` lines (written when a session ends or continues in a new file) hold
  per-model session totals — input, output, thinking, cache read, cache write, cost — that include
  subagents: the STP-1 to STP-3 session shows 197,599 output and 118,515 thinking tokens for the reviewer
  model `claude-fable-5-1`, against about 2,000 imported.
- The notice that ends a subagent carries `subagent_tokens`, which equals the context of its last call plus
  its final output (14 of 14 subagents of STP-4), not the sum over its calls.
- The current session has no `cost-state` line yet.

**Risk tags:** `data` (what a `measured` record means changes for subagents).

**Non-goals.** Cost in money; live metering while a session runs; providers other than Claude Code; changing
how Claude Code writes transcripts.

**Promise (closed list).** For Claude Code transcripts on this machine's format:

1. a subagent record never shows a streamed partial count as `measured`;
2. where only a total is known, it is recorded as such and marked, never split by guess;
3. session totals from `cost-state` lines can be imported per model and compared with the per-message
   records, so the gap is visible in `stapel tokens`.

Not promised: splitting a session total between tickets that share a session by time (only windows that
start and end at `cost-state` lines can be exact); usage of background calls no transcript holds.

**Size.** Small: one importer change, one report change, two or three criteria.

### Decisions

- 2026-10-05: the frame is confirmed by the human.

## Spec

TODO

## Design

TODO

## Proof

TODO

## Plan

TODO

## Review

TODO

## Summary

TODO
