---
id: 198
title: "Manual portable Backup, automatic safety Backup, and reminder"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Manual portable Backup, automatic safety Backup, and reminder

- Let the operator create a portable Parish Backup explicitly through **Transfer & Backup → Create parish backup**, choose its destination, and receive a read-back-verified `.ordo` file.
- Automatically create a locally managed **safety Backup** before a schema migration, an application upgrade that changes stored data, or a Parish Backup restore. Do not require ceremonial approval for this protective prerequisite.
- Do not treat a locally managed safety Backup as evidence that parish data exists off the computer. It never clears the portable-backup reminder.
- When authoritative Parish-scope content changed and no portable Backup has been successfully verified for seven days, show a non-blocking reminder in Transfer & Backup and the application header. Suppress it when nothing changed since the last portable Backup.
- Present the last successful portable-backup time and the number of authoritative changes since it. Clear the reminder only after successful destination read-back verification.
- Do not schedule writes to absent removable drives or attempt backups while Ordo is not running in the MVP. A missing USB device must not create repeated background errors or alerts.
