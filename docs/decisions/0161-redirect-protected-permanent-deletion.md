---
id: 161
title: Redirect-protected permanent deletion
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

# Redirect-protected permanent deletion

- Treat a Merged Song redirect as durable identity evidence for as long as its merge remains applied. Do not expose independent Archive or Delete actions for the redirect record.
- Block permanent deletion of a surviving Song while any retired Song UUID redirects to it, even if the survivor has no Arrangements or other ordinary references.
- Group incoming redirects as explicit blockers in the existing permanent-deletion safety dialog and link each one to its merge-history event.
- Require the operator to reverse the relevant merges first, thereby restoring the retired Songs and removing their redirects, before the survivor may otherwise qualify for permanent deletion.
- Never satisfy deletion by silently erasing redirects, rewriting historical UUIDs, or leaving old imports and internal links unresolved.
