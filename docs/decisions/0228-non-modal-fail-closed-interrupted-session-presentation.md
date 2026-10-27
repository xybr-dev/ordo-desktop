---
id: 228
title: Non-modal fail-closed interrupted-session presentation
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: []
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Non-modal fail-closed interrupted-session presentation

- After unexpected termination, reopen directly into the recovered native-full-screen Presenter workspace rather than adding a blocking recovery dialog before the operator can regain context.
- Show a persistent operator header strip: **Presentation recovered · Congregation output is black**. Restore Preview to the last selected Cue and show the last Program frame only as clearly private pre-interruption context; never claim that it is currently visible externally.
- Restore journaled navigation, Freeze/Blackout/Live intent, emergency inserts, and emergency overrides to the operator session while keeping actual external Program fail-closed on black.
- If the previously assigned congregation display is confidently recognized and output had been enabled, recreate its passive window black. If it cannot be recognized, enter the adopted Disconnected state without moving output to another display.
- Let Space take the valid recovered Preview live and dismiss the recovery strip. Keep **Exit Presenter** available so an old or no-longer-relevant interrupted session can end through the ordinary safe exit flow.
- Do not add a second Resume confirmation. The black external output, persistent recovered-state strip, deliberate Space action, and visible Exit preserve safety while minimizing recovery latency during an actual Mass.
