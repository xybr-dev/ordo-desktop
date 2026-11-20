//! A synthetic 500-Cue Presentation snapshot, written to disk on demand.
//!
//! This is measurement support, not product code: `ordo`'s
//! `live-control-lab --measure-memory` calls [`write_synthetic_snapshot`] to
//! build a snapshot in a scratch directory, so no 500-Cue fixture has to be
//! committed. The output is a `snapshot-manifest` v2 `snapshot.json` plus its
//! `frames/` and `assets/`, and it must pass the strict
//! [`crate::snapshot_reader::read_snapshot`] like any other snapshot; the unit
//! test below proves it does.
//!
//! The shape follows `tests/presentation/tests/frame_navigation.rs`: 6
//! sections, 60 Mass items (20 of 9 Cues and 40 of 8, exactly 500 Cues),
//! mixed Cue behaviours, 10 Unavailable Cues, and up to 120 distinct frames.
//! Six of the frames are General Image frames over two real PNG assets, so
//! decoded-image memory is exercised too.
//!
//! Every text is invented placeholder text; nothing here is liturgical,
//! scripture or hymn content.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub mod drill;

/// Cues in the synthetic snapshot.
pub const CUES: usize = 500;
const SECTIONS: usize = 6;
const ITEMS: usize = 60;
/// Distinct frames the Project Cues can use.
const DISTINCT_FRAMES: usize = 120;
/// Every `IMAGE_EVERY`th frame (from `IMAGE_FIRST`) is a General Image frame.
const IMAGE_EVERY: usize = 20;
const IMAGE_FIRST: usize = 3;
/// The two PNG assets: a larger and a smaller picture (width, height).
const ASSET_SIZES: [(u32, u32); 2] = [(1280, 720), (640, 360)];

/// What [`write_synthetic_snapshot`] wrote.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntheticSnapshot {
    /// Cues in the manifest.
    pub cues: usize,
    /// Mass items in the manifest.
    pub items: usize,
    /// Frames declared (every one is used by a Project Cue).
    pub frames: usize,
    /// General Image frames among them.
    pub image_frames: usize,
    /// PNG assets declared.
    pub assets: usize,
    /// Cues that are Unavailable.
    pub unavailable_cues: usize,
}

fn uuid(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

fn frame_uuid(n: usize) -> String {
    uuid(0x50_0000 + n as u64)
}

fn asset_uuid(n: usize) -> String {
    uuid(0x60_0000 + n as u64)
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn is_image_frame(n: usize) -> bool {
    n >= IMAGE_FIRST && (n - IMAGE_FIRST).is_multiple_of(IMAGE_EVERY)
}

/// The asset behind image frame `n`.
fn image_asset_of(n: usize) -> usize {
    (n / IMAGE_EVERY) % ASSET_SIZES.len()
}

/// The text of frame `n`: a General Image frame, or one of the nine text
/// kinds.
fn frame_text(n: usize) -> String {
    const KINDS: [&str; 9] = [
        "Lyrics",
        "Response",
        "ReadingIdentification",
        "ReadingPassage",
        "Psalm",
        "SongIdentification",
        "CelebrationTitle",
        "GeneralText",
        "Black",
    ];
    if is_image_frame(n) {
        let asset = image_asset_of(n);
        let (width, height) = ASSET_SIZES[asset];
        return format!(
            "ordo-fixture/1\nkind:GeneralImage\nheader:Placeholder header {n}\nbody-lines:0\nfooter-lines:1\nPlaceholder footer {n}\nimage:{}\nimage-size:{width}x{height}\n",
            asset_uuid(asset),
        );
    }
    let kind = KINDS[n % KINDS.len()];
    if kind == "Black" {
        return "ordo-fixture/1\nkind:Black\nheader-absent\nbody-lines:0\nfooter-absent\nimage-absent\n"
            .to_owned();
    }
    format!(
        "ordo-fixture/1\nkind:{kind}\nheader:Placeholder header {n}\nbody-lines:2\nPlaceholder line {n} alpha\nPlaceholder line {n} beta\nfooter-lines:1\nPlaceholder footer {n}\nimage-absent\n"
    )
}

/// Writes the synthetic 500-Cue snapshot under `dir` (created if missing) and
/// returns what it holds. Deterministic: the same call writes the same bytes.
pub fn write_synthetic_snapshot(dir: &Path) -> io::Result<SyntheticSnapshot> {
    let kinds = [
        "song",
        "reading",
        "psalm",
        "response_sequence",
        "general_presentation",
        "rite",
    ];
    let theme = uuid(0x400);
    let mut flat = 0usize;
    let mut used = [false; DISTINCT_FRAMES];
    let mut unavailable_cues = 0;
    let mut sections = Vec::new();
    for section in 0..SECTIONS {
        let mut items = Vec::new();
        for item in section * ITEMS / SECTIONS..(section + 1) * ITEMS / SECTIONS {
            // 20 items of 9 Cues and 40 of 8: exactly 500.
            let size = if item < 20 { 9 } else { 8 };
            let mut cues = Vec::new();
            for _ in 0..size {
                let (behavior, context) = match flat {
                    i if i % 50 == 7 => {
                        unavailable_cues += 1;
                        (
                            json!({"type": "unavailable", "reason": "placeholder asset missing"}),
                            None,
                        )
                    }
                    i if i % 23 == 5 => (
                        json!({"type": "operator_trigger"}),
                        Some("placeholder trigger"),
                    ),
                    i if i % 31 == 11 => (json!({"type": "hold"}), None),
                    i if i % 37 == 13 => (json!({"type": "wait"}), Some("placeholder wait")),
                    i => {
                        let frame = i * 7 % DISTINCT_FRAMES;
                        used[frame] = true;
                        (
                            json!({"type": "project", "frame_id": frame_uuid(frame)}),
                            None,
                        )
                    }
                };
                let mut cue = json!({
                    "id": uuid(0x1_0000 + flat as u64),
                    "label": format!("Cue {flat}"),
                    "behavior": behavior,
                });
                if let Some(context) = context {
                    cue["private_context"] = json!(context);
                }
                cues.push(cue);
                flat += 1;
            }
            items.push(json!({
                "id": uuid(0x1000 + item as u64),
                "title": format!("Item {item}"),
                "kind": kinds[item % kinds.len()],
                "cues": cues,
            }));
        }
        sections.push(json!({
            "id": uuid(0x100 + section as u64),
            "title": format!("Section {section}"),
            "items": items,
        }));
    }
    debug_assert_eq!(flat, CUES);

    fs::create_dir_all(dir.join("frames"))?;
    fs::create_dir_all(dir.join("assets"))?;
    let mut digests = Vec::new();
    let mut frames = Vec::new();
    let mut image_frames = 0;
    let mut assets_used = [false; ASSET_SIZES.len()];
    for n in (0..DISTINCT_FRAMES).filter(|n| used[*n]) {
        let text = frame_text(n);
        let path = format!("frames/frame-{n:03}.txt");
        fs::write(dir.join(&path), &text)?;
        let sha256 = sha256_hex(text.as_bytes());
        frames.push(json!({"id": frame_uuid(n), "path": path, "sha256": sha256}));
        digests.push(sha256);
        if is_image_frame(n) {
            image_frames += 1;
            assets_used[image_asset_of(n)] = true;
        }
    }
    let mut assets = Vec::new();
    for (index, (width, height)) in ASSET_SIZES.iter().enumerate() {
        if !assets_used[index] {
            continue;
        }
        let bytes = png(*width, *height, index as u32);
        let path = format!("assets/picture-{index}.png");
        fs::write(dir.join(&path), &bytes)?;
        let sha256 = sha256_hex(&bytes);
        assets.push(json!({
            "id": asset_uuid(index),
            "path": path,
            "size": bytes.len(),
            "sha256": sha256,
        }));
        digests.push(sha256);
    }

    // The dependency fingerprint: every digest's hex, sorted, each followed by
    // LF, hashed once more.
    digests.sort_unstable();
    let mut fingerprint = Sha256::new();
    for digest in &digests {
        fingerprint.update(digest.as_bytes());
        fingerprint.update(b"\n");
    }

    let manifest: Value = json!({
        "schema_version": 2,
        "id": uuid(1),
        "mass_plan_id": uuid(2),
        "celebration_id": uuid(3),
        "captured_at": "2026-11-01T09:30:00Z",
        "title": "Synthetic 500-Cue Mass",
        "themes": [{"id": theme, "name": "Fallback", "kind": "fallback"}],
        "mass_theme_id": theme,
        "sections": sections,
        "frames": frames,
        "assets": &assets,
        "dependencies_fingerprint": hex(&fingerprint.finalize()),
    });
    let mut text = serde_json::to_string_pretty(&manifest).map_err(io::Error::other)?;
    text.push('\n');
    fs::write(dir.join("snapshot.json"), text)?;
    Ok(SyntheticSnapshot {
        cues: CUES,
        items: ITEMS,
        frames: frames.len(),
        image_frames,
        assets: assets.len(),
        unavailable_cues,
    })
}

/// A valid 8-bit RGB PNG with a deterministic gradient, written with stored
/// (uncompressed) deflate blocks so no encoder dependency is needed. `seed`
/// shifts the pattern so two pictures differ.
fn png(width: u32, height: u32, seed: u32) -> Vec<u8> {
    let mut raw = Vec::with_capacity((width as usize * 3 + 1) * height as usize);
    for y in 0..height {
        raw.push(0); // filter type: none
        for x in 0..width {
            raw.push(((x + seed * 64) % 256) as u8);
            raw.push(((y * 255 / height.max(1)) % 256) as u8);
            raw.push(((x ^ y).wrapping_add(seed * 31) % 256) as u8);
        }
    }
    let mut zlib = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65_535).peekable();
    while let Some(block) = blocks.next() {
        zlib.push(u8::from(blocks.peek().is_none()));
        let len = block.len() as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for byte in data {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot_reader::{DirectorySource, read_snapshot};

    #[test]
    fn the_generated_snapshot_passes_the_strict_reader() {
        let dir =
            std::env::temp_dir().join(format!("ordo-synthetic-snapshot-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let summary = write_synthetic_snapshot(&dir).unwrap();
        let loaded = read_snapshot(&DirectorySource::new(&dir));
        let _ = fs::remove_dir_all(&dir);
        let loaded = loaded.expect("the synthetic snapshot loads");
        assert_eq!(loaded.snapshot().cue_count(), CUES);
        assert_eq!(loaded.snapshot().item_count(), ITEMS);
        assert_eq!(summary.cues, CUES);
        assert_eq!(summary.unavailable_cues, 10);
        assert!(summary.frames >= 100, "{summary:?}");
        assert_eq!(summary.image_frames, 6);
        assert_eq!(summary.assets, 2);
    }
}
