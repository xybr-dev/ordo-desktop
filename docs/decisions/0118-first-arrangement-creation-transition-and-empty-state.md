---
id: 118
title: First-Arrangement creation transition and empty state
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# First-Arrangement creation transition and empty state

- Use the same adaptive Arrangement-details slideover for both creation and editing. In creation it is titled **New Arrangement**, omits existing-record actions such as Duplicate Arrangement, and changes the bottom commit to **Create Arrangement**; in editing it is titled **Arrangement details** and commits with **Save details**.
- Validate the adopted minimum of one language before creation. Keep the display title and all other Arrangement metadata optional under Decision [117](0117-arrangement-metadata-boundary-and-staged-details-surface.md).
- Commit creation explicitly. Do not persist an abandoned or invalid empty Arrangement merely because the slideover was opened.
- After **Create Arrangement**, select the newly created Arrangement and land directly in its **Lyrics** task.
- Treat an Arrangement with no sections as a valid authoring state. Show **No sections yet** and make **Add first section** the focused next action.
- Do not manufacture a default **Verse 1** section. Kyrie, Gloria, chants, refrains, and other forms do not share one reliable opening-section convention.
- Keep the empty Arrangement visible in the Song overview if the operator leaves before adding lyrics. It remains a deliberate created record rather than being silently deleted.
