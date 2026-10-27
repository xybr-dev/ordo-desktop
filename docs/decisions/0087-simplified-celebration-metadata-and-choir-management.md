---
id: 87
title: Simplified Celebration metadata and Choir management
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, mass-plan]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Simplified Celebration metadata and Choir management

- The MVP deliberately omits liturgical grade or rank from Celebration metadata. The shared Celebration fields remain date, title, controlled season, liturgical color, and flexible parish applicability tags; specialized structures are selected through templates and tags rather than a detailed precedence engine.
- **Songs → Choirs** manages lightweight Choir records with a required name and optional short label, preferred languages, private notes, and Active or Archived state. Member rosters, attendance, rehearsals, and contacts remain out of scope.
- Each Choir exposes its simple Song repertoire and the applicable Arrangements beneath each Song. Assigning a Choir in a Mass plan filters the picker toward that repertoire without silently choosing content.
- One exact Arrangement may be associated with multiple Choirs. If Choir A and Choir B sing the same Kyrie wording, sequence, and pagination, both Choirs point to the same Song work and Arrangement instead of duplicating either.
- Archiving a Choir removes it from ordinary new-plan selection while preserving its historical Mass-plan and Arrangement references.
