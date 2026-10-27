---
id: 135
title: Automatic-first Song import review and content comparison
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, transfer]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Automatic-first Song import review and content comparison

- Validate every selected Song Import Document before changing the library. Keep all imported content staged and read-only until the operator commits reviewed actions.
- Classify each candidate automatically as **Add new Song**, **Attach Arrangement**, **Skip unchanged**, or **Needs decision**. Explain each suggestion with concrete normalized identity, attribution, metadata, lyric, Sequence, and pagination evidence as applicable.
- Exclude contract-invalid documents before commit and report their precise validation failures. Invalid candidates remain inspectable in the review session but never contribute a library mutation.
- Require the operator to resolve every **Needs decision** candidate before committing the batch. Allow any suggested action to be changed deliberately during review.
- Commit all approved mutations in one transaction. If any approved action fails, save none of the batch; skipped and invalid candidates do not participate in that transaction.
- Preview the imported document from its comparison panel. When credible library candidates exist, open the same read-only, full-app, viewport-bounded **Compare contents** slideover rather than separate preview surfaces.
- In that slideover, compare the imported document with one selectable nearest library match side by side. Provide **Overview**, **Lyrics**, **Sequence**, and **Slides** views, preserve the underlying review selection and staged action, and return without editing either side.
- Keep the nearest-match selector visible only when one or more credible candidates exist. Comparison is evidence for an import decision, never an automatic merge editor.
