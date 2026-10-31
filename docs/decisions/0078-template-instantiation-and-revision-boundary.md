---
id: 78
title: Template instantiation and revision boundary
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Template instantiation and revision boundary

- Creating a Mass plan from a template materializes an independent copy of the selected template revision into that plan.
- Later edits to the reusable template affect future plan creation only. They never silently add, remove, reorder, or rewrite items in an existing Mass plan.
- A Mass plan may retain the originating template and revision as provenance, but that reference does not imply synchronization.
- Merging or reapplying later template changes into an already edited Mass plan is deferred beyond the MVP because its conflict behavior would require a distinct reviewed workflow.
