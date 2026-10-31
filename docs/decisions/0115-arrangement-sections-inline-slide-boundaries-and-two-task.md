---
id: 115
title: "Arrangement sections, inline slide boundaries, and two task tabs"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, mass-plan, rendering]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Arrangement sections, inline slide boundaries, and two task tabs

- Use exactly two primary task tabs in the focused Arrangement editor: **Lyrics** and **Sequence**. Remove the provisional separate **Slides** tab.
- Treat an Arrangement section as a reusable musical/lyrical unit such as Verse 1, Chorus, Refrain, Bridge, or Ending. One section may generate one or several projectable slides without becoming several sequence entries.
- Keep exact lyric encoding, intentional line breaks, automatic slide proposals, manual slide-boundary adjustment, canonical generated-slide thumbnails, fit warnings, and pagination approval together inside **Lyrics** for the selected section.
- Represent a slide boundary as presentation structure inside its owning section. Moving or removing the boundary changes slide generation but never creates, removes, or reorders a section occurrence.
- Do not require artificial sections such as Verse 1A and Verse 1B merely because Verse 1 needs two slides. Such labels remain allowed when they describe genuinely distinct musical or lyrical parts.
- Keep **Sequence** focused on ordered section occurrences. One Verse 1 occurrence expands to all of Verse 1's generated slides, and repeated Chorus occurrences continue referencing the same reusable Chorus section.
- Keep the authoritative canonical Inspector visible across both tabs. Selecting a generated slide in Lyrics or a section occurrence in Sequence drives that Inspector.
- Do not retain a dedicated Pagination Review workspace at either Arrangement or Mass-plan level in the MVP. Imported `Needs Review` boundaries and later overflow findings open the exact affected content in its owning editor.
