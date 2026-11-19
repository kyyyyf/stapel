---
name: stapel-elaborate
description: "Turns the intent for a piece of work larger than one ticket into requirements with ids and a breakdown into tickets."
# stapel: 0.1.0 sha256:fa674e82340aca9b146767cabb9824fa2230355080ee0aaf1405454c8b535fd7
---

# Elaborate a piece of work larger than one ticket

## When it applies

The person describes a piece of work as larger than one ticket: it needs several tickets, or its
requirements are not yet known. A single ticket does not need this skill; use the skill `stapel` for it.
The result is a draft of requirements and a breakdown into tickets. No stapel command reads the draft yet.

## Questions

Your task is to elicit the person's intent, never author it. The person knows what they want; you do not.

- Ask at most four questions in one dialog, each with options and a recommendation.
- Ask only what the draft needs and the text of the person does not answer.
- Put each answer in the section Decisions as a `Decision` line with the date.
- When the person does not answer a point, write an `Open question` line. Never invent an answer.
- When you assume a fact, write an `Assumption` line and say so.

## Steps

1. Choose a short slug from the title: lowercase words joined with `-`.
2. Write the draft to `.stapel/features/<slug>.md`, copied from `template.md` without the first line of
   template.md (the line is the marker of stapel). Keep every section; write `None` in a section that is empty.
3. Fill Description, Outcome, Done when (three to six checks), Boundaries (at least one thing out) and
   Constraints from what the person said.
4. Write the requirements as lines `R<n>`. A requirement says what is possible and what shows it was met.
   It says WHAT, never HOW. Ids count up from `R1`; an id is never reused or renumbered, even when its
   requirement is removed.
5. Write the Breakdown: one line per ticket, at most 12 Plan steps each. Each line names the requirements
   it covers (`covers`) and the tickets it comes after (`after`). Every requirement is covered by at least
   one ticket.
6. Next, run checklist.md over the draft, check by check.
7. Fix what fails. List what you cannot fix in the chat, and name each failing check by its `EL` id.
8. Last, name the path of the draft in the chat, so that the person reads it in the editor.

## Rules

- Write in English, in short sentences, with one term for one meaning.
- Keep the sections of the template in their order and add none.
- Do not start a ticket from the draft. The person starts each ticket later with `stapel new`.
