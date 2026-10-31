---
id: 89
title: "Lightweight Presider, Location, and notes input"
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

# Lightweight Presider, Location, and notes input

- **Presider** and **Location** are editable comboboxes in Mass details: they suggest parish defaults and previously used values while allowing the operator to enter a new value immediately. A visiting priest or diocesan bishop can therefore be assigned directly to one Mass without first becoming a managed parish record.
- The MVP does not add a clergy directory or location-management workspace. **Parish Settings → Defaults** owns only a lightweight list of **Regular Presiders**, an optional **Default Presider**, and an optional **Default Mass location**. Private preparation notes remain unrestricted multiline text owned by the individual Mass plan.
- A newly created Mass plan copies the optional Default Presider and Default Mass location as its initial values, and the operator may immediately replace either. Regular Presiders appear first in the editable Presider combobox; other previously used names remain available as recent suggestions without silently becoming Regular Presiders.
- The parish default is not a live reference. Changing it affects only Mass plans created afterward and never rewrites historical or already-prepared plans.
- Duplicating a plan under the same Celebration or reusing it for another Celebration retains the source plan's saved Presider and Location instead of replacing either with the current parish defaults.
