# STP-22 — stapel init installs the process: skill, agent definitions, hooks

## Description

Source: `docs/PHASES.md`, plan item 1.3; the ledger decision of 2026-10-21 (approval dialogs, part 2: rule 5
of the spec process moves into the skill text).

### Frame

**Problem.** The process lives in this repository's `CLAUDE.md` and in briefs the orchestrator writes by
hand, so a foreign repository gets the guard from `stapel init` but no process. `stapel init` installs the
process as data: an orchestrator skill, four agent definitions and a SessionStart hook that points the main
agent to the skill; a re-run updates stapel's own files and keeps the person's.

**Plan item:** 1.3.

**Risk tags:** `interface` (files Claude Code reads: skill, agents, settings), `guard` (the tools and
permission mode of each agent; reviewers read-only), `migration` (a re-run updates installed files in a
repository that already has them, and keeps hand edits).

**Non-goals.** Tokens from hooks (item 1.4); the review pack and triage (item 1.5); one dialog per ticket
(item 1.8); new guard rules (items 1.9, 1.10); a `--force` flag or a `.new` file for an edited file; a
plugin or marketplace package; files under the user's `~/.claude`; removing installed files; any agent
other than Claude Code; making the model follow the skill text.

**Promise (closed list).**

1. `stapel init` writes `.claude/skills/stapel/SKILL.md` and `.claude/agents/stapel-builder.md`,
   `stapel-drift.md`, `stapel-reviewer.md`, `stapel-spec-reviewer.md`. Each file's YAML frontmatter holds
   the marker line `# stapel: <version> sha256:<hex>`, the hash taken over the file without that line.
2. Each agent file has `name`, `description`, `tools`, `model` and `permissionMode`. `model` is the
   `model` of its role in `[models]` (builder, drift, reviewer; the spec reviewer takes `reviewer`), left
   out when the role is missing. `permissionMode` defaults to `acceptEdits` for the builder and `default`
   for the others, and a `permission_mode` key of the role overrides it. Reviewers and the drift reviewer
   get only `Read, Grep, Glob`; the builder gets `Read, Edit, Write, Bash, Grep, Glob`. A value Claude Code
   does not accept refuses with exit 1 before any write.
3. On a re-run: a missing file is created; a file whose marker hash matches its text is rewritten when
   stapel's text differs (`updated:`) and left alone when equal; a file whose hash does not match, or that
   has no marker, is left alone with a warning that names it. A re-run with nothing to change prints
   `already set up: nothing changed`, as today.
4. `init` adds a SessionStart hook to `.claude/settings.json` next to every other hook. Its command
   `stapel hook session-start` prints one line of context that tells the main agent to load the skill
   `stapel` and run `stapel next`; a missing or failing binary prints nothing and lets the session start.
5. The skill text holds the general process of this repository's `CLAUDE.md` (the rules of "How work is
   done here", "The spec process" items 1 to 9 with rule 5 as changed on 2026-10-21, "Build and review"),
   the loop "run `stapel next`, do what it names, run it again", and the rule "run `stapel brief` on every
   step before the spec review". Each agent file's body holds that role's working rules.
6. In this repository the last step runs `stapel init`, and `CLAUDE.md` keeps only what belongs to this
   repository (the plan, language, layout, private terms, test thread limits, commit author) with one line
   that names the skill; the human applies the `CLAUDE.md` change with a script.

Not promised: a hand edit that keeps the hash; a file whose line endings git changed (it reads as edited);
hiding the marker line from the model; a skill or agent named `stapel*` that the person wrote without a
marker is kept, not merged; deliberate circumvention.

**Prior art.** The builder briefs and reviewer prompts of STP-5 to STP-21 (`.stapel/scratch/stp5-r1-brief.md`,
the agent calls in the transcripts); `init.rs` (`plan_hooks`, `plan_create`, the upgrade of the bare hook
line); the local SessionStart hook of this repository (`.claude/settings.local.json`); the Claude Code
documentation of subagents and skills (fetched 2026-10-21: `model` takes a full id, unknown frontmatter
fields are ignored, a subagent cannot ask the person, files are picked up without a restart).

**Size.** Estimated about 350 changed source lines (`crates/stapel-cli`: `init.rs`, the session-start
hook; `crates/stapel-core`: `permission_mode` in the model binding), about 250 lines of skill and agent
text as files under `crates/stapel-cli/`, and about 700 test lines. Past about 600 source lines the ticket
is split before the spec review. Route: full (tags), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-22: Decision (marker, the human, recommended): a YAML comment line `# stapel: <version>
  sha256:<hex>` in the frontmatter, the hash over the file without that line; a matching hash means
  stapel's unchanged file, which a re-run updates; a mismatch or no marker means the person's file, which
  is kept. Rejected: a version alone (a hand edit is lost on update); never overwrite (a new stapel never
  updates its files).
- 2026-10-22: Decision (loading the skill, the human, recommended): a SessionStart hook prints one line
  that names the skill and `stapel next`, because Claude Code cannot be made to load a skill and `init`
  does not touch the person's `CLAUDE.md`. Rejected: a marked block in `CLAUDE.md`; the skill description
  alone.
- 2026-10-22: Decision (model and permission mode, the human, recommended): `model` from the role in
  `[models]`, the spec reviewer on `reviewer`; `permissionMode` by default `acceptEdits` for the builder
  and `default` for the read-only agents, overridden by a `permission_mode` key of the role. Rejected: a
  new `[agents]` table (two places for a model); values fixed in the code.
- 2026-10-22: Decision (this repository's `CLAUDE.md`, the human, recommended): the last step installs the
  skill here and moves the general process out of `CLAUDE.md`, applied by the human with a script.
  Rejected: keep both texts (they drift); leave this repository without the skill.
- 2026-10-22: Decision (scope of the skill, the human, recommended): the general rules of "How work is
  done here", items 1 to 9 and "Build and review" go into the skill; `CLAUDE.md` keeps the plan, language,
  layout, private terms, thread limits and commit author. Rejected: items 1 to 9 only (a foreign
  repository would lack RED/GREEN and the review rules).
- 2026-10-22: Decision (an edited file, the human, recommended): it is kept and `init` warns with its name;
  to take stapel's new text the person deletes the file and runs `init` again. Rejected: a `.new` file next
  to it; a `--force` flag.

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
