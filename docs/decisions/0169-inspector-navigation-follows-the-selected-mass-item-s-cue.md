---
id: 169
title: "Inspector navigation follows the selected Mass item's Cue order"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, ui]
supersedes: []
superseded_by: null
amends: [11]
amended_by: []
sources: []
---

# Inspector navigation follows the selected Mass item's Cue order

- This narrowly supersedes Decision [11](0011-program-preview-freeze-and-blackout-semantics.md)'s ambiguous operator-only Preview
  wording: the canonical Preview frame is black, while private prompt, role,
  timing, and related-response context remain outside it.
- Navigate the Inspector through the selected Mass item's flattened **Cue** order rather than counting only congregation-facing slides. Label position as **Cue n of total** so operator-only triggers are represented honestly.
- Show a projectable Cue as its authoritative canonical congregation frame. Show an operator-only trigger as a black congregation frame with its private minister role, prompt, and timing context outside the frame; operator-only text may never appear inside the canonical output.
- Keep editor and Inspector selection synchronized in both directions. Clicking a generated Cue focuses it in the Inspector, while Inspector Previous/Next selects and reveals the corresponding Cue in the owning editor hierarchy.
- Stop navigation at the first and last Cue of the selected Mass item and disable the unavailable direction. Do not silently select an adjacent Mass item; changing item scope remains an explicit Mass-outline action.
- Preserve the full `16:9` authoritative frame while showing warning, source, Theme, and private operator context around it. Use contextual **Fix pagination** only for affected projectable content.
- Treat `ordo-inspector-cue-navigation*` (now [`docs/mockups/inspector-cue-navigation.html`](../mockups/inspector-cue-navigation.html), Decision [271](0271-mockups-are-one-page-per-screen-with-a-status-each.md)) as the approved interaction and information-hierarchy reference for this behavior.
