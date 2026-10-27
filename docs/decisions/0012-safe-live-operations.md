---
id: 12
title: Safe live operations
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: []
supersedes: []
superseded_by: null
amends: []
amended_by: [273]
sources: []
---

# Safe live operations

> **Amended by Decision [273](0273-f-and-b-toggle-and-releasing-either-takes-preview-live.md).** `B` now toggles Blackout; releasing `F` or `B` does what `Space` does.

While presenting, operators may:

- Navigate forward/back.
- Freeze and unfreeze.
- Blackout and restore.
- Jump to another prepared Mass item.
- Select a prepared alternate.
- Insert emergency text or an image.

Ordinary editing of lyrics, canonical responses, and plan structure is locked during live presentation.

Emergency text/image insertion is the exception:

- Opening the tool, including through the `Insert` key, automatically Freezes Program so unfinished content cannot leak to the congregation.
- Saving places the new cue immediately after the current cue and selects its canonical rendered frame in Preview.
- `Space` takes the inserted cue live using the ordinary Freeze-to-Live behavior.
- The insert is autosaved inside the current Mass plan for crash recovery.
- It does not enter the permanent song/content library unless someone explicitly promotes it after Mass.

Emergency editing is a second controlled exception:

- `Ctrl+E` opens the selected projectable cue for editing and automatically Freezes Program.
- Program retains its previous frame while edits update only the canonical Preview.
- The edit is autosaved as a Mass-local override and never silently overwrites the song library or imported/official liturgical text.
- `Space` takes the corrected Preview live.
- When the slide comes from a reused song section, the override updates every linked occurrence of that section in the current Mass by default.
- The operator may explicitly limit an override to the selected occurrence.
- After Mass, Ordo may offer the override for reviewed promotion into the canonical library/source.

## 12a. MVP keyboard map

- `Left` or `Up`: select the previous cue.
- `Right` or `Down`: select the next cue.
- `Page Up` or `Page Down`: jump to the previous or next Mass-item group.
- `Home` or `End`: jump to the first or last cue in the Mass plan.
- `F`: toggle Freeze.
- `B`: activate Blackout.
- `Space`: take the selected Preview live and clear Freeze or Blackout.
- `Esc`: close the current dialog or overlay without altering Program.
- `Insert`: open the emergency text/image insert workflow and automatically Freeze Program.
- `Ctrl+E`: emergency-edit the selected projectable cue and automatically Freeze Program.
- Navigation always follows the established state semantics: it updates Preview and Program while Live/unfrozen, but only Preview while Frozen or Blacked out.
- Shortcut customization is deferred until real operator usage provides evidence for it.
