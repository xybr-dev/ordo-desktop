---
id: 220
title: Empty Mass Templates within the permanent list-editor workspace
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

# Empty Mass Templates within the permanent list-editor workspace

- Preserve the ordinary Mass Templates list-editor workspace before the first template exists. Do not replace it with a temporary onboarding page or a special-purpose setup wizard.
- Show **No templates yet** with a concise explanation that a Mass template provides reusable Mass sections, recurring responses, and typed Song/Reading slots for one language.
- Make **Create template** the primary empty-state action. Creation asks only for the template name and language, then opens the existing Template Editor with the adopted six-section scaffold: Before Mass, Introductory Rites, Liturgy of the Word, Liturgy of the Eucharist, Concluding Rites, and After Mass.
- Treat the six sections as editable structural starting points only. Do not speculate about language-specific responses, readings, rubrics, songs, or other liturgical content during creation.
- Keep template creation behind one Template Module interface so the empty-state action and every later **New template** action invoke the same validation, creation semantics, and editor transition.
- Continue allowing a Mass plan to be created without a template. If neither an applicable template nor a language fallback exists, open the already-adopted empty plan with its visible readiness warning rather than forcing template setup.
