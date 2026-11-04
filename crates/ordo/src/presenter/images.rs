//! Image decoding for the Presenter's frame store (T1.4g): the full and the
//! thumbnail-sized decode, and the image check Preflight builds on.
//!
//! Every function here is pure apart from decoding and runs off the GPUI
//! thread; the caller spawns it on the background executor.

use std::fmt;
use std::io::Cursor;
use std::sync::Arc;

use domain::presentation::AssetId;
use gpui::{Image, ImageFormat, RenderImage, SvgRenderer};
use image::{DynamicImage, ImageDecoder, ImageReader};
use presentation::snapshot_reader::LoadedSnapshot;

/// The longest side, in pixels, of a thumbnail-sized decode. A grid
/// thumbnail is a few hundred device pixels wide at most, so 640 px keeps it
/// sharp on a high-density display while a 4K slide background costs about
/// a ninth of its full decode. Only frames wanted solely as thumbnails
/// (`FrameSlot::Visible`) use it; Program, Preview and Preload decode in full.
pub const THUMBNAIL_IMAGE_MAX_PX: u32 = 640;

/// Why one asset could not become a usable image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageCheckError {
    /// The snapshot holds no bytes for the asset.
    Missing,
    /// The bytes are not a decodable PNG.
    Undecodable,
}

impl fmt::Display for ImageCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("the asset is missing from the snapshot"),
            Self::Undecodable => f.write_str("the asset is not a decodable image"),
        }
    }
}

impl std::error::Error for ImageCheckError {}

/// Decode `bytes` at full size into the `RenderImage` GPUI paints.
pub(super) fn decode_full(
    bytes: &[u8],
    renderer: &SvgRenderer,
) -> Result<Arc<RenderImage>, ImageCheckError> {
    Image::from_bytes(ImageFormat::Png, bytes.to_vec())
        .to_image_data(renderer.clone())
        .map_err(|_| ImageCheckError::Undecodable)
}

/// Decode `bytes` and shrink it so its longest side is at most
/// [`THUMBNAIL_IMAGE_MAX_PX`] (a smaller image is kept as is; the aspect
/// ratio is preserved). Applies the EXIF-style orientation and the BGRA
/// channel order exactly as GPUI's own decode does. The full-size pixels
/// exist only transiently inside this call, on the background thread.
pub(super) fn decode_thumbnail(bytes: &[u8]) -> Result<Arc<RenderImage>, ImageCheckError> {
    let undecodable = |_| ImageCheckError::Undecodable;
    let mut decoder = ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png)
        .into_decoder()
        .map_err(undecodable)?;
    let orientation = decoder.orientation().map_err(undecodable)?;
    let mut decoded = DynamicImage::from_decoder(decoder).map_err(undecodable)?;
    decoded.apply_orientation(orientation);
    if decoded.width().max(decoded.height()) > THUMBNAIL_IMAGE_MAX_PX {
        decoded = decoded.thumbnail(THUMBNAIL_IMAGE_MAX_PX, THUMBNAIL_IMAGE_MAX_PX);
    }
    let mut pixels = decoded.into_rgba8();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])))
}

/// Decode-check every asset of `snapshot` once and return each failure.
///
/// The building block for Preflight's image check (T1.6): it proves, before
/// Mass, that every image a frame can place will decode. Each asset is
/// decoded in full and the result is dropped before the next asset, so at
/// most one decoded image is alive at a time. The order follows
/// [`LoadedSnapshot::asset_ids`]. Pure apart from decoding; it blocks for
/// the whole decode, so the caller must run it off the GPUI thread.
pub fn check_images(
    snapshot: &LoadedSnapshot,
    renderer: &SvgRenderer,
) -> Vec<(AssetId, ImageCheckError)> {
    check_with(snapshot, |asset| {
        let bytes = snapshot.asset(asset).ok_or(ImageCheckError::Missing)?;
        decode_full(bytes, renderer).map(drop)
    })
}

/// How many assets `snapshot` holds, for "N checked".
pub fn image_count(snapshot: &LoadedSnapshot) -> usize {
    snapshot.asset_ids().count()
}

fn check_with(
    snapshot: &LoadedSnapshot,
    mut check: impl FnMut(AssetId) -> Result<(), ImageCheckError>,
) -> Vec<(AssetId, ImageCheckError)> {
    snapshot
        .asset_ids()
        .filter_map(|asset| check(asset).err().map(|error| (asset, error)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presenter::frames::test_support::fixture_snapshot;

    #[test]
    fn a_good_snapshot_has_no_image_failures() {
        let snapshot = fixture_snapshot();
        assert!(image_count(&snapshot) > 0);
        let renderer = SvgRenderer::new(Arc::new(()));
        assert_eq!(check_images(&snapshot, &renderer), Vec::new());
    }

    #[test]
    fn corrupt_bytes_are_one_failure_naming_the_asset() {
        let snapshot = fixture_snapshot();
        let bad = snapshot.asset_ids().next().expect("fixture has assets");
        let failures = check_with(&snapshot, |asset| {
            let bytes: &[u8] = if asset == bad {
                b"not a png"
            } else {
                snapshot.asset(asset).expect("asset present")
            };
            decode_full(bytes, &SvgRenderer::new(Arc::new(()))).map(drop)
        });
        assert_eq!(failures, vec![(bad, ImageCheckError::Undecodable)]);
    }

    #[test]
    fn thumbnail_decode_caps_the_longest_side_and_keeps_aspect() {
        let snapshot = fixture_snapshot();
        for asset in snapshot.asset_ids() {
            let bytes = snapshot.asset(asset).expect("asset present");
            let full = decode_full(bytes, &SvgRenderer::new(Arc::new(()))).unwrap();
            let thumb = decode_thumbnail(bytes).unwrap();
            let (fw, fh) = (full.size(0).width.0 as u32, full.size(0).height.0 as u32);
            let (tw, th) = (thumb.size(0).width.0 as u32, thumb.size(0).height.0 as u32);
            assert!(tw.max(th) <= THUMBNAIL_IMAGE_MAX_PX);
            if fw.max(fh) <= THUMBNAIL_IMAGE_MAX_PX {
                assert_eq!((tw, th), (fw, fh), "a small image is not resized");
            } else {
                assert!(tw.max(th) == THUMBNAIL_IMAGE_MAX_PX);
            }
        }
    }

    #[test]
    fn a_large_image_shrinks_to_the_thumbnail_bound() {
        let wide = image::RgbaImage::from_pixel(1280, 720, image::Rgba([10, 20, 30, 255]));
        let mut png = Vec::new();
        wide.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let thumb = decode_thumbnail(&png).unwrap();
        assert_eq!(
            (thumb.size(0).width.0, thumb.size(0).height.0),
            (640, 360),
            "longest side is capped and the aspect ratio is kept"
        );
        let full = decode_full(&png, &SvgRenderer::new(Arc::new(()))).unwrap();
        assert_eq!(full.size(0).width.0, 1280);
        // Opaque input stays opaque, in BGRA order (B first).
        assert_eq!(&thumb.as_bytes(0).unwrap()[..4], &[30, 20, 10, 255]);
    }

    #[test]
    fn thumbnail_decode_rejects_corrupt_bytes() {
        assert_eq!(
            decode_thumbnail(b"not a png").err(),
            Some(ImageCheckError::Undecodable)
        );
    }
}
