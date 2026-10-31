---
id: 75
title: Conditional template items
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

# Conditional template items

- A parish Mass template may contain conditional Mass items whose default inclusion is evaluated from Celebration metadata such as season and explicit Celebration tags.
- Evaluation is automatic-first but advisory rather than an unreviewable universal authority. Included and omitted results remain visible in the preparation outline with their reason; omitted items do not enter Presenter Mode.
- The operator may explicitly include an item omitted by a template rule or omit an included item for the current Mass plan. A plan-level override does not silently rewrite the reusable template.
- When required metadata is missing, rules conflict, or Ordo cannot determine the applicable rubric, the item remains visibly unresolved and contributes a readiness warning instead of being silently included or removed.
- These rules are parish-owned template knowledge. Ordo may ship or import reviewed starting material later, but the core application does not hardcode every liturgical exception as unquestionable behavior.
