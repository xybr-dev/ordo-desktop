---
id: 76
title: Typed template slots and multiple templates per language
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

# Typed template slots and multiple templates per language

- A Mass template may mix concrete reusable content with typed unresolved slots. Initial slot types include **Song slot** and **Reading slot**; a slot preserves the expected liturgical role and placement without pretending its plan-specific content has already been chosen.
- Song slots may be required, optional, or conditional. When instantiated for a Mass plan, their chooser is filtered by the assigned choir and relevant applicability metadata, but Ordo does not silently select or replace a Song merely because it is in that choir's repertoire.
- Reading slots represent roles such as First Reading, Second Reading, and Gospel rather than assuming one fixed count for every Celebration. Operators may insert additional Reading slots into an individual plan, while a dedicated template may encode a recurring exceptional structure when warranted.
- A language may have any number of parish templates. Templates are distinguished by their own name, purpose, and advisory applicability—not by language alone—and the operator may deliberately choose among them.
- This flexible template model must not be mistaken for qualified MVP support for every sacrament or special rite. Funeral, baptism, wedding, Easter Vigil, and other dedicated templates can be added by the parish or developed later without changing the basic Mass-item and slot model.
