---
id: 27
title: Optional content-source extensions
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

# Optional content-source extensions

- Sulugoon integration is an optional content-source extension rather than part of Ordo's core domain.
- Ordo core owns Mass plans, editable parish content, manual entry, and the normalized import-and-review workflow.
- A content-source extension converts external material into Ordo's source-neutral import draft. Imported material becomes an ordinary parish copy and does not depend on the extension remaining installed or available.
- The Sulugoon extension supplies Cebuano source material without making Sulugoon's structure or language assumptions canonical for English, Tagalog, or future providers.
- The Sulugoon extension will not ship with public Ordo releases. It may exist as a private local extension for the product owner's personal use and must remain architecturally separate from the core application.
- MVP does not require a runtime that loads executable third-party extensions. A private source-specific helper operates outside Ordo and emits a versioned, source-neutral Liturgy Import Document.
- Ordo validates and previews a Liturgy Import Document before copying its content into a Mass plan.
- The same import contract may be used by future English, Tagalog, or other source-specific tools without changing Ordo core.
- A general third-party extension marketplace or user-facing extension installer is deferred.
