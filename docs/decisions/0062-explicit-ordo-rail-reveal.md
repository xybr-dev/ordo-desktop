---
id: 62
title: Explicit Ordo-rail reveal
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Explicit Ordo-rail reveal

- Presenter Mode never opens the collapsed Ordo rail merely because the pointer hovers at an edge. Reveal requires an intentional click or the rail's keyboard command, preventing accidental overlays during live operation.
- A narrow, clearly labeled **ORDO** tab remains visible while the rail is collapsed so its location and purpose are discoverable without occupying a full column.
- Activating the tab opens the unpinned rail as an overlay over the Cue timeline. This temporary reveal does not resize thumbnails, alter their column count, scroll the timeline, or change Preview, Program, or selection.
- The rail's Pin control converts the overlay into the persistent first column. Unpinning restores the collapsed-tab behavior and returns the reclaimed space to the timeline.
- Clicking outside an unpinned rail or pressing `Esc` closes it and restores keyboard focus to the previously active Cue. Closing the rail never changes presentation state.
- The exact rail keyboard binding remains part of the final shortcut-map review; the interaction contract does not depend on a particular key.
