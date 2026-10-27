---
id: 218
title: Minimal first-run Parish setup with regular-Presider defaults
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

# Minimal first-run Parish setup with regular-Presider defaults

- On an installation with no Parish scope, show one focused **Set up Ordo** page with two paths: **Start a new parish** and **Restore a parish backup**. Do not use a tutorial carousel or generic onboarding dashboard.
- Starting a new Parish asks for a required Parish name, optional Default Mass location, and optional Regular Presiders. If exactly one Regular Presider is entered, select that person as the Default Presider automatically; with several, let the operator explicitly choose one default or leave the default unassigned.
- Keep Regular Presiders a lightweight Parish-default suggestion list rather than a clergy directory. New Mass plans copy the Default Presider, while every Presider field remains an editable combobox that prioritizes Regular Presiders, shows recent guest names separately, and accepts an immediate custom value for a visiting priest or bishop.
- Creating the Parish generates its Parish UUID and opens the empty Masses workspace. Do not request an account, internet connection, telemetry consent, content language, Choir setup, Theme choice, display assignment, or other optional configuration during first run.
- Initial restore enters the existing full-page restore review, verifies the Backup, and adopts the Backup's Parish identity. Because no current Parish scope exists, do not create a meaningless pre-restore safety Backup; all later restores retain the adopted recoverability requirement.
- Do not provide **Skip setup** because Ordo requires a Parish identity. Keep setup to one meaningful decision surface and allow optional fields to remain blank.
- After new-Parish creation, the empty Masses workspace offers **Create first Celebration**, **Import `.ordo`**, and **Create or manage templates** as practical next actions without forcing any one of them.
