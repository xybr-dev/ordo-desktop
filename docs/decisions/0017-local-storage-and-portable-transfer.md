---
id: 17
title: Local storage and portable transfer
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [storage, transfer]
supersedes: []
superseded_by: null
amends: []
amended_by: [180, 268]
sources: []
---

# Local storage and portable transfer

> **Superseded in part by Decision [268](0268-make-ordo-windows-only-for-the-mvp-and-defer-linux.md).** Preparation happens on a connected Windows machine, not Linux. The transfer workflow is otherwise unchanged.

> **Superseded in part by Decision [180](0180-celebration-scoped-portable-transfer.md).** The original single-plan Mass Package
> became the Celebration-scoped Package reflected in the reconciled text below.
> Local-first storage, Parish Backup, and reviewed transfer rules remain.

- Ordo is local-first and fully functional without internet access.
- The church presentation PC is currently air-gapped. Preparation may occur on an internet-connected Linux computer and be carried to the Windows church PC by USB.
- Ordinary preparation edits autosave immediately to a local database; there is no general Save button that operators must remember to press.
- Live presentation recovery continues to use the more detailed continuous journal described in section 14.
- Cloud or LAN synchronization is not required for the MVP, but stable record identities and import boundaries must not prevent synchronization later.
- Ordo supports two distinct portable package types:
  - A **Celebration Package** is the ordinary deployment and return-transfer unit for one complete Celebration. Exporting it automatically captures or reuses one immutable Presentation snapshot for every Mass plan currently under that Celebration and includes every dependency needed to present those plans offline, including song arrangements, rendered slides, theme assets, images, liturgical content and source references, and fonts when redistribution is permitted. Shared dependencies are stored once by content identity where possible.
  - A **Parish Backup** is a complete snapshot for disaster recovery or migration to another computer. It contains the parish library, choirs, templates, Mass plans, themes, source references, settings, and media.
- The normal deployment workflow is:
  1. Prepare the Celebration and its scheduled Mass plans on the internet-connected Linux computer.
  2. Export a self-contained Celebration Package to removable media.
  3. Import and review it on the air-gapped church PC.
  4. Select and present each included Mass plan entirely offline.
  5. If emergency inserts or Mass-local overrides should be retained, export the resulting Celebration Package back to Linux for post-Mass review and possible promotion.
- Package imports match records by stable IDs rather than titles alone and never silently replace conflicting library content.
- A Celebration Package presents every included Mass plan from that plan's own embedded immutable snapshot and dependencies. These snapshots install as exact presentation content independently of any editable-library reconciliation.
- These per-plan snapshot boundaries ensure that the church PC renders every included Mass exactly as captured on the preparation computer, even when its permanent library contains older, newer, or locally changed records.
- The same import review separately stages editable source records for the receiving parish library. New identities, unchanged identities, safe fast-forwards, and genuinely divergent records are classified before one reviewed commit; accepting or rejecting a library action cannot alter any Presentation snapshot inside the package.
- Restoring a Parish Backup is replace-only in the MVP; Ordo does not attempt to merge two complete parish databases.
- Before restore, Ordo automatically creates a recoverable safety backup of the current installation and verifies the incoming package before replacing local data and assets as one consistent unit.
- If post-restore verification fails, the operator can roll back to the automatically created safety backup.
