---
id: 69
title: Song/Arrangement picker slide-over
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, rendering, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Song/Arrangement picker slide-over

- Choosing or replacing a Song/Arrangement opens a floating slide-over from the right, visually above the underlying Mass planner. The underlying planner retains its place and selection.
- Search results and Arrangement details may scroll beneath a sticky slide-over header. Choosing an Arrangement stages it; it does not mutate the Mass item until the operator explicitly commits.
- The sticky header's top-right primary action identifies the staged result, for example **Use Choir A arrangement**. At constrained widths its visible label may shorten to **Use arrangement** while its accessible name and adjacent selected-Arrangement context remain explicit.
- A floating `›` control is attached to the slide-over's left boundary and closes the panel in the direction it retreats. It has the accessible name and tooltip **Close Song picker**; `Esc` performs the same dismissal.
- Closing without committing discards only the temporary picker selection. It does not change the existing Mass item, outline position, inspector state, or canonical slides.
- The commit action is not duplicated at the bottom of the desktop slide-over, avoiding competing primary actions and keeping it continuously reachable above long results or details.
