---
id: 222
title: Protected fallback-only Theme Library state
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [themes]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Protected fallback-only Theme Library state

- Never present the Theme Library as literally empty. Always show a **System Theme** section containing the protected **Ordo Black** safety Theme with its canonical specimen preview and visible **Built in** status.
- When no parish-owned Theme exists, show **No parish themes yet** in a separate **Parish Themes** section and make **Create parish theme** its primary action. Treat this as a valid ready-to-present state rather than an error or onboarding blocker.
- Create a parish Theme as an independent copy of Ordo Black and open it immediately in the existing staged Theme Editor. This guarantees a complete renderable starting state while allowing every property to diverge safely.
- Route **Create parish theme** and explicit duplication of Ordo Black through the same Theme Module interface and revision-safe creation semantics. The operator-facing wording may reflect the initiating context, but identity generation, asset copying, and editor handoff remain one implementation.
- Keep Ordo Black as the Parish default until the operator explicitly sets another Theme as default. It remains selectable and duplicable but cannot be edited in place, renamed, archived, or deleted.
- Show **Parish default** on whichever Theme currently owns that role. On first use, Ordo Black therefore displays both **Built in** and **Parish default** without pretending it is parish-owned content.
