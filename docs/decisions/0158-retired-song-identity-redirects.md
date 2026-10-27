---
id: 158
title: Retired Song identity redirects
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Retired Song identity redirects

- After merge, retain the retiring Song's UUID as an internal **Merged Song redirect** rather than treating the retired identity as an archived Song or leaving it as an independently selectable library record.
- Resolve every lookup of that UUID to the current surviving Song. Exclude the redirect from ordinary Song Library results, but let searches for an old title or alias return the survivor with a visible **Matched merged identity** explanation.
- Route old internal links to the survivor and briefly disclose that the requested Song was merged. Do not strand bookmarks, History links, import evidence, or other internal navigation on a dead identity.
- Resolve a Song import document that names a retired UUID to the survivor automatically while disclosing the redirect during import review. Write the survivor's current UUID in all new exports and references.
- When a surviving Song is merged again, flatten every earlier redirect so each retired UUID points directly to the newest survivor. Reject any operation that would introduce a redirect loop.
- Retain the merge provenance needed to reverse an eligible merge and reconstruct the prior redirect relationships atomically.
