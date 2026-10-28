---
id: 257
title: Make Library a multi-view destination with revision-bound reuse
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [storage]
supersedes: [112]
superseded_by: null
amends: [64, 71, 168, 250]
amended_by: [278]
sources: []
---

# Make Library a multi-view destination with revision-bound reuse

> **Amended by Decision [278](0278-manage-choirs-as-a-library-view.md) (proposed).** If accepted, the Library gains a fourth view, **Choirs**, for the Choir management workspace of Decisions 87 and 221. The rest of this record stands.

- This supersedes Decisions [64](0064-preparation-shell-primary-navigation.md)/[250](0250-use-a-hybrid-global-rail-and-local-tab-navigation-hierarchy.md)'s Songs destination label and Decision [112](0112-mass-ownership-of-template-derived-response-sequences.md)'s
  Mass-owned template Response copies, and supersedes Decision [71](0071-add-mass-item-workflow-and-editor-presets.md)'s separate
  Custom text/Announcement presets plus Decision [168](0168-owner-directed-pagination-correction-instead-of-a-review.md)'s Custom text editor label
  with General Presentation.
- Rename the top-level Songs destination to **Library** with local views
  **Songs**, **Responses & Prayers**, and **General Presentations**. Keep Song
  and Arrangement as their specialized domain rather than forcing every entry
  into one generic content schema.
- Response/Prayer and General Presentation entries are reusable sources. A Mass
  item references one until **Customize for this Mass** creates a local override;
  Mass templates retain these links when instantiated.
- A General Presentation Mass item may add private delivery context without
  changing the reusable source. That context remains operator-only and never
  enters a canonical frame.
- One Mass plan owns one reusable-source binding per source. Every still-linked
  occurrence uses that binding's Edit revision. Reusing the source again uses
  the same bound revision; a newer upstream revision appears as **Update
  available** until reviewed, and advancing updates every still-linked
  occurrence atomically. Local overrides remain unchanged.
- A local override remembers its source and base revision and exposes compare
  and replace-with-latest behavior when upstream changes. Presenter sessions pin
  resolved revisions; Preview, Program, and upcoming Cues never hot-refresh.
  Historical Presentation snapshots remain immutable.
- Promoting a local override never auto-merges across a changed upstream. Review
  latest source beside local content and save an explicit new source revision.
  Advance the current plan binding on success; relink the promoted occurrence
  only when it exactly matches the saved source. Other plans receive **Update
  available**.
- Archiving never breaks a bound occurrence and is not by itself a readiness
  failure. Hide archived entries from selection by default and expose them with
  **Include archived**. Restore before ordinary source editing. Permanent delete
  is limited to never-referenced entries with no retained history.
