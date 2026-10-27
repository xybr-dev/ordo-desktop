---
id: 179
title: Safe planned exit from Presenter Mode
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Safe planned exit from Presenter Mode

- Keep **Exit Presenter** visible in the Presenter global header and provide no single-key exit shortcut that could end a live presentation accidentally.
- When congregation output is disabled, exit immediately. When output is enabled, open a centered **End presentation?** confirmation while Program remains unchanged; initially focus **Stay in Presenter** and suspend single-key Presenter commands according to the adopted modal rule.
- On confirmed exit, force Program to black before closing the congregation-output window. Then mark the Presentation session complete and clear its interrupted-session recovery marker so the next launch does not reopen a deliberately ended session.
- Restore the operator window's pre-Presenter bounds and the Mass Planner's prior item/section selection, scroll positions, pane state, and section disclosure state.
- Preserve every saved Mass-local emergency insert or override. Planned exit ends the session but never reverses preparation or emergency changes.
- Keep unexpected termination under the existing interrupted-session recovery model; it is not interpreted as a planned exit and always returns congregation output fail-closed on black.
