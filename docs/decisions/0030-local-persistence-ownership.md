---
id: 30
title: Local persistence ownership
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [storage]
supersedes: []
superseded_by: null
amends: []
amended_by: [252]
sources: []
---

# Local persistence ownership

> **Superseded in part by Decision [252](0252-replace-electron-with-a-clean-slate-rust-and-gpui.md).** SQLite, managed-asset consistency,
> domain-shaped persistence operations, and migration requirements remain; the
> Electron utility-process and `node:sqlite` implementation does not.

- Use one local SQLite database for structured parish data, including Songs, Arrangements, Mass plans, Themes, settings, edit revisions, and Presentation-snapshot manifests.
- A dedicated Electron utility process is the sole owner of the live database connection. React renderers never execute SQL or open the database directly, and the Electron main process does not perform database work on its event loop.
- The application reaches persistence through a small asynchronous Storage Module Interface exposed through narrow preload/IPC commands. SQL schemas, transactions, migrations, journal handling, and database-driver behavior remain local to the Storage Module implementation.
- Original images and other large binary assets live in a managed parish asset directory and are referenced by stable IDs and integrity checksums. Disposable thumbnails, decoded frames, and other generated caches are not durable database records.
- The Storage Module coordinates database and asset changes so callers cannot create a committed record that references a missing asset, or silently replace an existing asset with different bytes under the same identity.
- Parish Backups, Celebration Packages, and other portable artifacts are created through coordinated snapshot/export operations. Ordo never treats an uncoordinated copy of a live SQLite main file as a valid backup, because active journal files may form part of the database state.
- The utility-process cost is accepted to isolate persistence faults and keep database I/O away from the main, renderer, and live-presentation paths. Its memory usage remains inside the established total Ordo performance budget.
- Use Electron's bundled `node:sqlite` driver inside the storage utility process for the MVP.
- The synchronous driver Interface is contained inside that utility process and is exposed to the rest of Ordo only through the asynchronous Storage Module Interface, so database calls cannot block the Electron main thread or a renderer.
- Using the bundled driver avoids an external native addon's Electron ABI rebuild and packaging requirements. Ordo pins and qualifies the exact Electron/Node/SQLite combination used for a release rather than depending on whichever versions happen to be installed on the operating system.
- Database features required by Ordo—including foreign-key enforcement, transactions, prepared statements, migrations, integrity checks, and coordinated backup—must be exercised in packaged Windows and Linux acceptance tests before release.
- Do not add an ORM for the MVP. The Storage Module Implementation uses explicit parameterized SQL and ordered, versioned schema migrations.
- The Storage Module Interface exposes domain-shaped operations rather than generic table CRUD or raw query access. This preserves Depth and Locality: callers work with Ordo concepts while schema knowledge and query behavior remain concentrated inside the module.
