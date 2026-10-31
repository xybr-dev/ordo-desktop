---
id: 66
title: Ordo-rail keyboard access and focus return
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

# Ordo-rail keyboard access and focus return

- `O` is the Presenter shortcut for **Open/focus Ordo**. It is an access command rather than a pin-state toggle.
- When the Ordo rail is collapsed, `O` opens its unpinned overlay and moves focus to the Mass item containing the active Cue. When the rail is already pinned, `O` moves focus into the visible rail without changing its geometry.
- `Esc` closes an unpinned Ordo overlay and returns focus to the active Cue. If the rail is pinned, `Esc` returns Cue focus without collapsing or unpinning the rail.
- Pin and Unpin remain explicit visible actions. Repeatedly pressing `O` cannot unexpectedly resize the Cue timeline.
- The collapsed rail's labeled **ORDO** tab, the visible shortcut reference, and the rail footer expose the `O` command; discoverability does not depend on memorization.
- Opening, focusing, closing, pinning, or unpinning the rail never changes Cue selection, Preview, Program, Freeze, Blackout, output availability, or presentation state.
