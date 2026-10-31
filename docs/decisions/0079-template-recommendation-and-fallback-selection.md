---
id: 79
title: Template recommendation and fallback selection
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Template recommendation and fallback selection

- Creating a Mass plan uses its language and Celebration applicability metadata to rank compatible Mass templates. The best match is preselected but remains visibly identified and changeable before creation; the operator is never locked to the recommendation.
- A parish may designate one general fallback template per language. A more specifically applicable template outranks that fallback, while deliberate manual selection remains allowed.
- If neither an applicable template nor a language fallback exists, Ordo creates an empty Mass plan with a visible readiness warning rather than silently choosing an unrelated template.
- Choir assignment does not select the Mass template. The template defines the Mass structure; the chosen choir's Arrangements subsequently fill or resolve Song slots.
