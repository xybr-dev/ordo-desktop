---
id: 173
title: Mass sections use local identity and editable labels only
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

# Mass sections use local identity and editable labels only

- Give every Mass section its own stable UUID and editable display name, without a hidden permanent liturgical type or behavior-bearing taxonomy.
- Let each language-specific template use its own section labels. Renaming a section changes its label without changing its items or causing Ordo to reinterpret its meaning.
- Keep conditional inclusion, liturgical applicability, and other planning behavior on the relevant Mass items, Template slots, and template metadata rather than inferring rules from section names.
- When deriving a plan in another language, use the target-language template's section structure rather than matching, translating, or synchronizing source section names.
- Treat custom and default-derived sections uniformly after creation; the default six are a convenience scaffold, not privileged runtime entities.
