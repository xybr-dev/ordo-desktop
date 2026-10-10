# Ordo packaging contract (for AI and offline tool authors)

Status: T0.8 WP2 spike contract, version 1. The schemas live in
`contracts/<contract>/v1.schema.json`; the Rust validators that mirror them live
in `tools/artifact-container-spike/src/contracts/`; the fixture corpus lives in
`tests/fixtures/contracts/`.

An Ordo file (`.ordo`) is a standard ZIP archive. Nothing here needs Ordo to
create one: any ZIP tool and any JSON tool are enough. Ordo never trusts the
extension. The verified `manifest.json` decides what the file is.

## 1. ZIP layout

```
manifest.json      required, the root manifest (JSON object)
payload.json       required, the kind's structured JSON payload
assets/...         optional, binary or text files referenced by the payload
```

Rules enforced on every archive:

- Entry names are relative, use forward slashes, and contain no `..`, no
  backslash, no drive prefix, no NUL, and no empty segment. Names are compared
  case-insensitively for duplicates.
- Directory entries (names ending in `/`) are allowed and carry no content.
  Symlinks and other non-regular entries are rejected.
- Every archive file must be listed in the manifest, and every listed file must
  be present. Unlisted entries are rejected.

## 2. Manifest fields

`manifest.json` is an object with exactly these members (no others are allowed):

| Member | Type | Rule |
|---|---|---|
| `format` | string | must be `"ordo-container"` |
| `format_version` | integer | must be `1` |
| `kind` | string | one of `song-import`, `liturgy-import`, `celebration-package`, `parish-backup`, `diagnostic-report` |
| `schema_version` | integer | the payload contract version; must be `1` in this build |
| `artifact_id` | string | UUIDv4, lowercase, hyphenated; stable identity of this file |
| `created_at` | string | RFC 3339 UTC, `YYYY-MM-DDTHH:MM:SSZ` |
| `payload` | object | `{ "path": "payload.json", "sha256": "<hex>", "size": <bytes> }` |
| `assets` | array | one `{ "path": "assets/...", "sha256": "<hex>", "size": <bytes> }` per asset file; may be empty |

`recovery-journal` is a local file, not a container kind. Its manifest entry does
not exist.

## 3. Hashing rule

- `sha256` is the **lowercase hexadecimal** SHA-256 digest (64 characters) of the
  **exact uncompressed bytes** stored in the archive. Do not hash the compressed
  bytes, do not re-serialize JSON, and do not normalize line endings before
  hashing. The payload hash covers `payload.json` byte for byte.
- `size` is the exact uncompressed byte count of that file.
- Hashes appear in three places: the manifest (container checksum of each file),
  the payload (a content hash of data inside the payload, defined per contract in
  section 6), and nowhere else.

## 4. Kinds and contracts

| Kind | Payload contract | Notes |
|---|---|---|
| `song-import` | `song-import` | one Song work with Arrangements (decision 40) |
| `liturgy-import` | `liturgy-import` | readings and responses for one Celebration |
| `celebration-package` | `celebration-package` | embeds `snapshot-manifest` objects |
| `parish-backup` | `parish-backup` | database file plus assets |
| `diagnostic-report` | `diagnostic-report` | always opened read-only |

`snapshot-manifest` and `recovery-journal` have schemas but are not container
kinds: the first is embedded in a Celebration Package, the second is a local file.

## 5. Versioning and unknown versions

- `format_version` and `schema_version` are integers. This build supports only
  `1` for both.
- An archive whose `schema_version` is higher than this build supports, or whose
  `kind` this build does not know, is refused with the `Unsupported` outcome and a
  compatible-version message such as
  `requires an Ordo version supporting song-import v2; this build supports v1`.
  It is never guessed or partially read. The payload validator reports a
  supported-kind version mismatch as code `unsupported-version`.
- A `format_version` greater than `1` is refused with the `Unsupported` outcome,
  as kind `container`, supported `v1`, and the same compatible-version message
  style: `requires an Ordo version supporting container v2; this build supports v1`.
- A `format_version` that is not an integer, or is `0`, a wrong `format` string,
  or an unknown or missing manifest member is a bad manifest (`Invalid`).

## 6. Payload contracts: shape and references

Every payload object has `schema_version: 1` and a stable `id` (a UUIDv4, or
UUIDv4 identities for the addressable records inside it). Every object forbids
unknown members. The shape is defined in the schema; the rules below are the
checks beyond shape.

| Contract | Internal references (`missing-reference`) | Hash rule (`hash-mismatch`) |
|---|---|---|
| `song-import` | each Arrangement `sequence` entry names a slide of the same Arrangement | `arrangements[].content_sha256` = SHA-256 of every slide line in slide order, each followed by a single LF |
| `liturgy-import` | a response `follows` names a reading | `readings[].text_sha256` and `responses[].text_sha256` = SHA-256 of that text's exact UTF-8 bytes |
| `snapshot-manifest` | each item `asset_ids` entry names an asset of the same snapshot | `assets_fingerprint` = fingerprint of all `assets[].sha256` |
| `celebration-package` | every snapshot asset is listed in the package `assets` with identical path, size, and sha256 (a missing id is `missing-reference`, a difference is `hash-mismatch`) | `package_version.fingerprint` = fingerprint of the snapshot `id`s |
| `parish-backup` | `parish.logo_asset_id`, when present, names an asset | `assets_fingerprint` = fingerprint of `database.sha256` and all `assets[].sha256` |
| `diagnostic-report` | each log `asset_id` names an asset of the report | `assets_fingerprint` = fingerprint of all `assets[].sha256` |
| `recovery-journal` | `cue_position.last_entry_seq` equals the last entry `seq` (0 when there are none) | `entries_fingerprint` = SHA-256 of one line per entry, in order: `seq`, TAB, `kind`, TAB, `cue_index`, LF |

**Fingerprint rule.** Used by `snapshot-manifest`, `celebration-package`,
`parish-backup`, and `diagnostic-report`: take the listed values, sort them by
their bytes, write each followed by a single LF, and take the SHA-256 of that text
(lowercase hex). An empty list hashes the empty string
(`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`).

Payload hashes cover data the payload itself carries. The bytes of a referenced
asset or database file are checked by the container layer against the manifest
entry and the payload's `sha256`.

Identifiers: UUIDs must be lowercase, hyphenated, and version 4. Any other form is
a `schema` error.

## 7. Error codes

| Code | Meaning |
|---|---|
| `schema` | missing, unknown, or mistyped member; malformed UUID, hash, date, or path; empty required array |
| `unsupported-version` | `schema_version` is not supported by this build |
| `missing-reference` | an internal reference names no record in the document |
| `hash-mismatch` | a recorded hash or fingerprint differs from the recomputed value |

Each finding carries a JSON Pointer (RFC 6901) to the offending value. Every
problem is collected in one pass. Semantic rules run only after the shape is
clean.

## 8. Limits

The spike's inspection limits (`InspectLimits::default()` in `src/inspect.rs`,
used for every import and for reopen verification in `create`):

| Limit | Value |
|---|---|
| archive size (`max_archive`) | 64 MiB |
| entries (`max_entries`) | 4096 |
| one uncompressed entry (`max_entry`) | 1 GiB |
| total uncompressed (`max_total`) | 4 GiB |
| compression ratio (`max_ratio`) | 200 |

The ZIP format constants (`lib.rs`): `FORMAT` = `ordo-container`,
`FORMAT_VERSION` = 1, `MANIFEST_PATH` = `manifest.json`, `PAYLOAD_PATH` =
`payload.json`, `ASSETS_PREFIX` = `assets/`, `SUPPORTED_VERSIONS` = `[1]`.

Parish Backups are temporarily capped at 64 MiB like every other kind. This
is an owner decision; revisit it when a per-kind archive limit is needed.

Exceeding a limit is an `Invalid` outcome, not a partial read. Entry content is
read through a bounded reader that stops one byte past the declared size.

## 9. Invalid versus Needs decision

- **Invalid** (`Inspection::Invalid`): the file fails its required container
  rules, integrity, dependency closure, or supported-version check, so it cannot
  take part in any import. Examples: a missing manifest, a bad path, a hash
  mismatch, a payload that fails its contract. Fix the file and try again.
- **Needs decision** (reconciliation layer only): the file is structurally valid
  but its identity or reconciliation outcome needs an operator choice, such as a
  duplicate Song work. The container layer never produces this outcome. It is a
  separate type so that Invalid is never confused with a judgement call.

## 10. Make a song-import `.ordo` by hand

Start from `contracts/song-import/examples/minimal.json`, which is a valid
payload. Replace its UUIDs with fresh UUIDv4 values (any UUID generator works,
for example `python -c "import uuid; print(uuid.uuid4())"`).

1. Write `payload.json`. Keep the file as UTF-8 without a BOM. Compute each
   Arrangement's `content_sha256` with the rule in section 6.
2. Compute the payload hash and size from the exact bytes on disk:
   - `sha256sum payload.json` (Git Bash) or
     `(Get-FileHash payload.json -Algorithm SHA256).Hash.ToLower()` (PowerShell)
   - `wc -c < payload.json` (Git Bash) or `(Get-Item payload.json).Length`
3. Write `manifest.json`:

   ```json
   {
     "format": "ordo-container",
     "format_version": 1,
     "kind": "song-import",
     "schema_version": 1,
     "artifact_id": "<fresh UUIDv4>",
     "created_at": "2026-10-10T09:00:00Z",
     "payload": { "path": "payload.json", "sha256": "<hash>", "size": <bytes> },
     "assets": []
   }
   ```

4. Zip the two files so that `manifest.json` and `payload.json` sit at the root of
   the archive, with forward-slash names. Any ZIP tool does this:
   - Info-ZIP: `zip -X -D song.ordo manifest.json payload.json`
   - Python: `python -c "import zipfile; z=zipfile.ZipFile('song.ordo','w',zipfile.ZIP_DEFLATED); z.write('manifest.json','manifest.json'); z.write('payload.json','payload.json'); z.close()"`

   Do not use Windows PowerShell 5.1 `Compress-Archive`: it can write backslash
   entry names, which the inspector rejects.
5. Check the file with the spike CLI from the repository root:

   ```
   cargo run -p artifact-container-spike -- inspect song.ordo
   ```

   A valid file prints the kind and the staged asset count. An invalid one lists
   each failure with its JSON Pointer.

## 11. The spike CLI

```
cargo run -p artifact-container-spike -- create song-import contracts/song-import/examples/minimal.json out.ordo
cargo run -p artifact-container-spike -- inspect out.ordo
```

`create` validates the payload, writes `manifest.json` and the payload into a
temporary sibling of `out.ordo`, reopens and verifies it, and only then renames it
into place. A failure leaves any existing `out.ordo` untouched. The CLI takes no
asset arguments; the library `create` function does.

`inspect` extracts only into a temporary staging directory. It never writes
anywhere else, and it has no Parish argument.

## 12. Fixtures for authors

For each contract, `contracts/<contract>/examples/minimal.json` is a copy of the
minimal valid fixture. The full corpus is in `tests/fixtures/contracts/<contract>/`:

- `minimal.valid.json`, `representative.valid.json`, `boundary.valid.json`
- `invalid.reject.json`, `unknown-version.reject.json`,
  `missing-reference.reject.json`, `tampered-hash.reject.json`, each with a sibling
  `.expect` file naming the expected code

Run `cargo test -p artifact-container-spike contracts` to check the corpus and the
schemas.
