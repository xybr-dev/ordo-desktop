---
id: 14
title: Interrupted-session recovery
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Interrupted-session recovery

- During presentation, Ordo continuously journals the active Mass, selected cue, Program frame, Preview frame, Freeze/Blackout/Live state, and emergency inserts/overrides.
- After an unexpected termination, Ordo immediately reopens the interrupted Mass in Presenter Mode and restores whether congregation output had been enabled.
- If congregation output had been enabled, Ordo recreates its window on a confidently recognized assigned display but restarts Program black; if it had been disabled, no congregation window is created. Ordo never automatically reprojects stale or unintended content after a crash.
- The operator screen restores the last Program frame as context, the Preview selection, and the interrupted state.
- Pressing `Space` takes the recovered Preview live and resumes the ordinary linked state.
