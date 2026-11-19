---
name: stapel-elaborate
description: "Turns the intent for a piece of work larger than one ticket into requirements with ids and a breakdown into tickets."
# stapel: 0.1.0 sha256:02054754f8ceb0f4431441e97bd50674d0551d9634904aa6341d273e96f52b79
---

# stapel-elaborate

Use this skill when the person describes a piece of work larger than one ticket. Elicit the person's
intent; never author it. Ask at most four questions per dialog, each with options and a recommendation.

Steps:

1. Write the draft from `template.md` to `.stapel/features/<slug>.md`.
2. Run `checklist.md` over it.
3. Fix what fails or list it.
4. Name the path in the chat.
