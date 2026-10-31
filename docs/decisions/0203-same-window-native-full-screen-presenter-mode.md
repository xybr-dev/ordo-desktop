---
id: 203
title: Same-window native-full-screen Presenter Mode
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter]
supersedes: []
superseded_by: null
amends: [10]
amended_by: []
sources: []
---

# Same-window native-full-screen Presenter Mode

- This supersedes Decision [10](0010-presenter-layout.md)'s former maximized/non-exclusive Presenter
  behavior. Entering **Presenter Mode** transforms the existing main operator
  window into native full-screen **Live Control** on the operator display; it
  does not create a dedicated third operating-system window or leave a hidden
  preparation window running alongside it.
- Before transformation, retain the operator window's bounds, maximized/full-screen state, current route, Mass Planner selection, scroll positions, pane state, and section disclosure state. Preparation UI is replaced within the same window while its state remains restorable.
- Keep **Live Control** as the canonical operator-facing screen inside Presenter Mode, not a separate mode or window. Render its confirmations, readiness surfaces, and operational slide-overs within the operator window rather than creating additional operating-system windows.
- The only possible second Ordo window is the passive borderless full-screen congregation-output window on the assigned display. Do not create it merely by entering Presenter Mode; create it only when the operator explicitly enables congregation output.
- On planned exit, force Program black when output exists, close the congregation window, leave native full-screen, restore the exact preparation workspace and previous operator-window state, and preserve saved emergency changes.
- When output is disabled or no second display exists, Presenter Mode consists solely of the one full-screen operator window. Display disconnection closes or invalidates congregation output without disturbing the operator window.
- Keep **Exit Presenter** visibly available and retain the adopted guarded-exit interaction. Native full-screen changes window presentation, not command semantics, crash recovery, or fail-closed Program behavior.
