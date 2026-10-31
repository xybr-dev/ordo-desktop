---
id: 31
title: Windows installation scope
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [platform]
supersedes: []
superseded_by: null
amends: []
amended_by: [252]
sources: []
---

# Windows installation scope

> **Superseded in part by Decision [252](0252-replace-electron-with-a-clean-slate-rust-and-gpui.md).** The conventional x64 installer,
> offline complete-installer update, data preservation, checksum, and signing
> policy remain. Electron Forge is no longer the packaging implementation.

- Package Ordo with Electron Forge.
- Ship the Windows MVP as a conventional x64 per-machine installer rather than a portable executable.
- Install application binaries under the normal protected per-machine application location and keep durable parish data outside the installation directory.
- Installation and application updates require Windows administrator approval. Administrator access is available on the initial church PC.
- Application upgrades must preserve the parish database, managed assets, settings, snapshots, and backups.
- A portable Windows distribution is deferred to avoid ambiguous duplicate installations and split parish-data ownership.
- MVP application updates use the same complete Windows installer as initial installation. There is no background updater, update server, or custom Ordo update-bundle format.
- The release is downloaded on an internet-connected preparation machine, copied to removable media, and run manually with Ordo closed on the air-gapped church PC.
- On the first launch of a newer application version, Ordo creates a pre-migration Parish Backup and verifies database integrity before applying ordered schema migrations.
- A failed migration must not destroy or partially replace the prior parish state; Ordo retains the pre-migration backup and presents a recovery path.
- The internal single-parish MVP may use an unsigned installer. Each release publishes a SHA-256 checksum, which is verified on the internet-connected preparation machine before the installer is copied to removable media.
- The initial operator may need to acknowledge Windows' unrecognized-application warning during installation.
- Do not install a self-signed root certificate on the church PC merely to suppress that warning; its certificate-management and expanded-trust costs are not justified for the controlled MVP deployment.
- A CA-trusted Authenticode signature becomes a release requirement before Ordo is distributed beyond the product owner's controlled machines. Certificate acquisition is therefore deferred, not rejected.
