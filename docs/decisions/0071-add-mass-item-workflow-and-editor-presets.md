---
id: 71
title: Add Mass item workflow and editor presets
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan]
supersedes: []
superseded_by: null
amends: []
amended_by: [257]
sources: []
---

# Add Mass item workflow and editor presets

> **Superseded in part by Decision [257](0257-make-library-a-multi-view-destination-with-revision-bound.md).** **General Presentation** replaces the
> separate Custom text and Announcement presets while retaining optional
> Mass-local private delivery context. The remaining insertion behavior and
> specialized presets below stay adopted.

- **+ Add Mass item** opens a compact anchored dropdown from the Mass-outline header. Opening or dismissing it does not mutate the Mass plan.
- The dropdown states its insertion destination. Choosing a type immediately creates, selects, and autosaves a Mass-local item; ordinary Undo can reverse an accidental insertion without an extra confirmation step.
- By default, the new item is inserted immediately after the currently selected Mass item within that item's section. If a section header is the active insertion target, append within that section. If neither an item nor a section is active, require the operator to choose a destination section instead of silently appending to the end of the Ordo.
- Always state the complete insertion destination in the dropdown, such as **After Responsorial Psalm · Liturgy of the Word**, and allow it to be changed before choosing the item type. Creating the item selects it and expands its destination section.
- Per-item context actions will expose explicit **Insert before** and **Insert after** commands that open the same type chooser; their remaining wording and organization are still unresolved.
- The chooser groups **Song**, **Reading**, linked **Response/Prayer**, **General Presentation**, **Hold**, **Blackout**, and **Custom sequence**. Song proceeds into the adopted staged Song/Arrangement picker; the other choices open their corresponding selected-item editor.
- Reading keeps Scripture reference and reading text as operator context by default while separately encoding the closing prompt and congregation response; full-reading projection remains an explicit per-item option.
- Response sequence encodes ordered role-tagged Cues such as operator-only `P` prompts and projectable `T` responses.
- General Presentation supports reusable or Mass-local constrained Text/Image Slides and may pair them with Mass-local private delivery context that never enters Program or the reusable source.
- Hold creates an ordinary operator-only black Cue and does not latch the Blackout state. When Live its black output follows the normal linked Program/Preview rule; Freeze is used when the previous Program frame must remain visible.
- Blackout creates an explicit Cue that enters the persistent Blackout state; Program stays black while Preview may navigate until Space takes the selected Preview live.
- The flexible escape hatch is named **Custom sequence**, not **Blank item**. It starts as an incomplete titled Mass item and lets the planner assemble an unusual mixture of response, text-slide, operator-trigger, and Blackout Cues.
