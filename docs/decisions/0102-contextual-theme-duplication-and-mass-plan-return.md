---
id: 102
title: Contextual Theme duplication and Mass-plan return
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, themes]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Contextual Theme duplication and Mass-plan return

- **Duplicate Theme** opens a compact naming dialog prefilled from the source name, such as **Parish Dark copy**. The operator may rename it before creation.
- Successful duplication creates a new independent Theme from the complete staged result and switches the Theme Editor to that new Theme. Subsequent edits and saves affect the duplicate, not the source.
- When duplication began from top-level **Themes**, the new Theme remains open in the ordinary library editing context.
- When duplication began through a Mass plan's Theme chooser, **Back to Mass Theme** returns to the preserved chooser and stages the new Theme as that plan's candidate selection.
- Returning from duplication never applies the new Theme by itself. The operator must still commit the Mass Theme chooser, preserving the chooser's staged Theme and override-removal transaction boundary.
- Other Mass plans that inherit the source Theme remain attached to the source and are unaffected by duplication.
