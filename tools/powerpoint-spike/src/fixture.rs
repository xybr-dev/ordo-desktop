//! Deterministic fixture frames and the E2E fixture plan.

use crate::{Cue, DeckPlan, FRAME_HEIGHT, FRAME_WIDTH, MassItem};

/// The fixture deck: identification, lyrics, response, reading, Hold, Blackout.
pub fn fixture_plan() -> DeckPlan {
    DeckPlan {
        items: vec![
            MassItem {
                title: "Identification".to_owned(),
                cues: vec![Cue::Image { png: frame(1) }],
            },
            MassItem {
                title: "Entrance Song".to_owned(),
                cues: vec![
                    Cue::Image { png: frame(2) },
                    Cue::Image { png: frame(3) },
                    Cue::OperatorOnly {
                        notes: "Hold: wait for the procession".to_owned(),
                    },
                ],
            },
            MassItem {
                title: "Penitential Act response".to_owned(),
                cues: vec![Cue::Image { png: frame(4) }],
            },
            MassItem {
                title: "First Reading".to_owned(),
                cues: vec![
                    Cue::Image { png: frame(5) },
                    Cue::OperatorOnly {
                        notes: "Blackout: reader at ambo".to_owned(),
                    },
                ],
            },
        ],
    }
}

/// A deterministic 1920x1080 PNG: a solid colour from `seed` with vertical
/// stripes in a darker shade, so every seed hashes differently.
pub fn frame(seed: u8) -> Vec<u8> {
    let base = [
        seed.wrapping_mul(53),
        seed.wrapping_mul(97).wrapping_add(40),
        seed.wrapping_mul(29).wrapping_add(90),
    ];
    let stripe = [base[0] / 2 + 100, base[1] / 2 + 100, base[2] / 2 + 100];
    let width = FRAME_WIDTH as usize;
    let height = FRAME_HEIGHT as usize;
    let mut pixels = Vec::with_capacity(width * height * 3);
    for _ in 0..height {
        for x in 0..width {
            let colour = if (x / 96 + usize::from(seed)) % 2 == 0 {
                base
            } else {
                stripe
            };
            pixels.extend_from_slice(&colour);
        }
    }
    encode_png(FRAME_WIDTH, FRAME_HEIGHT, &pixels)
        .expect("encoding a frame into memory cannot fail")
}

/// Encodes 8-bit RGB pixels as a PNG.
pub(crate) fn encode_png(width: u32, height: u32, rgb: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
        writer
            .write_image_data(rgb)
            .map_err(|error| error.to_string())?;
        writer.finish().map_err(|error| error.to_string())?;
    }
    Ok(out)
}
