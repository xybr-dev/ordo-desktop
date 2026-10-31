---
id: 131
title: Stable three-pane Sequence workspace
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Stable three-pane Sequence workspace

- Reuse the Arrangement editor's established three-pane shell when switching from Lyrics to **Sequence**, preserving spatial continuity rather than navigating to a differently structured editor.
- Use the left pane as **Available sections**, showing each reusable section's name, generated-slide count, usage count, and an explicit **Add** action. Clicking or focusing the section name alone never mutates Sequence.
- Use the center pane for the ordered, expandable performance Sequence and its adopted occurrence insertion, repeat, reorder, and removal actions.
- Keep the right pane as the same authoritative canonical Inspector used by Lyrics. Selecting an occurrence previews its first generated frame by default; selecting an expanded child slide previews that exact frame.
- Apply the adopted contextual insertion rule when an Available-section **Add** action is invoked, then visibly select the resulting center-pane occurrence.
