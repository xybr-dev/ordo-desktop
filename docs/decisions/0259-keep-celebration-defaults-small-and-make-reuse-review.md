---
id: 259
title: Keep Celebration defaults small and make reuse review explicit
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

# Keep Celebration defaults small and make reuse review explicit

- Celebration owns one default title. A Mass plan inherits it and may override
  the projected title for another language or local wording; clearing restores
  inheritance. Do not add Celebration primary-language or localized-title maps.
- Reuse for another Celebration marks every copied Reading **Needs Review**,
  even when the language matches. Editing or reviewing clears one flag; **Mark
  all readings reviewed** clears the remainder after confirmation. Reading text
  stays language-specific Mass-plan content rather than Celebration metadata.
- **Next Mass** targets only the next scheduled sibling under the same
  Celebration. Fade to black, close the current session safely, and open that
  plan's preflight without exposing it automatically. Hide the action when no
  later sibling exists.
- Use **Mass plan** for the complete scheduled preparation and **Ordo** for its
  ordered sections/items. Avoid **Mass Ordo**.
