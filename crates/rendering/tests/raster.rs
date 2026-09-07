//! T0.4 PR 7: rasterize the completed logical frame at a target density.
//!
//! Every check goes through [`rendering::render_template`] (or [`rendering::render`])
//! plus [`rendering::rasterize`], never through raster internals. Density must
//! change pixel dimensions and sharpness only: the serialized logical
//! composition stays identical.

use std::collections::HashMap;

use rendering::{
    ForegroundImage, RasterError, TargetDensity, TemplateContent, TemplateKind, Theme, rasterize,
    render_template,
};

fn lyrics(body: &str) -> TemplateContent {
    TemplateContent {
        kind: TemplateKind::Lyrics,
        header: Some("Section label".to_owned()),
        body: body.to_owned(),
        footer: None,
        image: None,
    }
}

fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * width + x) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
}

fn assert_program_frame_shaped(width: u32, height: u32, rgba: &[u8]) {
    // The immutable RGBA seam every presentation consumer takes: non-empty
    // bytes with length a multiple of four, sized to the raster extent. The
    // `ordo` layer wraps exactly this into `ProgramFrame` without
    // `rendering` naming that type.
    assert!(width > 0 && height > 0);
    assert!(!rgba.is_empty(), "raster must carry pixels");
    assert_eq!(
        rgba.len() % 4,
        0,
        "raster length must stay RGBA-aligned: {}",
        rgba.len()
    );
    assert_eq!(
        rgba.len() as u64,
        u64::from(width) * u64::from(height) * 4,
        "raster bytes must match width x height"
    );
}

/// One decoded PNG for the image source, built through the qualified
/// `tiny-skia` edge so the test adds no image-codec dependency.
fn solid_png(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(width, height).expect("test pixmap must exist");
    pixmap.fill(tiny_skia::Color::from_rgba8(r, g, b, 255));
    pixmap.encode_png().expect("test png must encode")
}

#[test]
fn density_changes_pixels_not_composition() {
    let theme = Theme::default();
    let content = lyrics("Maayong buntag, mga igsoon!\nPagdayeg sa Ginoo — awit 95 α 中");
    let light_density = TargetDensity::from_percent(100).unwrap();
    let dense_density = TargetDensity::from_percent(200).unwrap();

    // Composition ignores density: same content and Theme serialize
    // byte-identically at both densities.
    let (at_light, _) = render_template(&content, &theme, light_density).unwrap();
    let (at_dense, _) = render_template(&content, &theme, dense_density).unwrap();
    assert_eq!(at_light.serialize(), at_dense.serialize());

    // One completed frame rasterized from, at both densities: dimensions
    // differ, logical composition is the same frame either way.
    let empty_images = HashMap::new();
    let light = rasterize(&at_light, light_density, &empty_images).unwrap();
    let dense = rasterize(&at_light, dense_density, &empty_images).unwrap();
    assert_eq!((light.width(), light.height()), (1920, 1080));
    assert_eq!((dense.width(), dense.height()), (3840, 2160));
    assert_eq!(light.logical_size(), (1920, 1080));
    assert_eq!(dense.logical_size(), (1920, 1080));
    assert_program_frame_shaped(light.width(), light.height(), light.rgba());
    assert_program_frame_shaped(dense.width(), dense.height(), dense.rgba());
    assert_ne!(light.rgba().len(), dense.rgba().len());
}

#[test]
fn text_leaves_ink_while_corners_stay_square_black() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let (frame, _) = render_template(&lyrics("Maayong buntag!"), &theme, density).unwrap();
    let raster = rasterize(&frame, density, &HashMap::new()).unwrap();
    let (width, height) = (raster.width(), raster.height());
    let rgba = raster.rgba();

    // Square canvas: every corner is opaque background, never a rounded
    // cutout or transparent pixel. The 4px operator radius never enters here.
    for (x, y) in [
        (0, 0),
        (width - 1, 0),
        (0, height - 1),
        (width - 1, height - 1),
    ] {
        assert_eq!(
            pixel(rgba, width, x, y),
            [0, 0, 0, 255],
            "corner ({x},{y}) must stay opaque background"
        );
    }
    // Real glyph ink somewhere inside: white text over black, so some pixel
    // must rise clearly above the background.
    let inked = rgba
        .chunks_exact(4)
        .any(|pixel| pixel[0] > 32 && pixel[3] == 255);
    assert!(inked, "a text slide must leave visible ink");
}

#[test]
fn black_rasterizes_all_black() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let black = TemplateContent {
        kind: TemplateKind::Black,
        header: None,
        body: String::new(),
        footer: None,
        image: None,
    };
    let (frame, diagnostics) = render_template(&black, &theme, density).unwrap();
    assert!(diagnostics.warnings.is_empty());
    let raster = rasterize(&frame, density, &HashMap::new()).unwrap();
    assert_eq!((raster.width(), raster.height()), (1920, 1080));
    assert!(
        raster
            .rgba()
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 0, 0, 255]),
        "a black slide paints nothing but opaque background"
    );
}

#[test]
fn foreground_images_resolve_at_raster_time() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let content = TemplateContent {
        kind: TemplateKind::GeneralImage,
        header: Some("Image header".to_owned()),
        body: String::new(),
        footer: Some("Image caption".to_owned()),
        image: Some(ForegroundImage {
            id: "test-asset-80x60".to_owned(),
            natural_width: 80,
            natural_height: 60,
        }),
    };
    let (frame, _) = render_template(&content, &theme, density).unwrap();
    let placed = frame.image().expect("frame carries geometry only");
    assert_eq!(placed.id, "test-asset-80x60");
    assert!(frame.serialize().contains("image id=test-asset-80x60"));

    let mut images = HashMap::new();
    images.insert(
        "test-asset-80x60".to_owned(),
        solid_png(80, 60, 200, 30, 20),
    );
    let raster = rasterize(&frame, density, &images).unwrap();
    assert_eq!((raster.width(), raster.height()), (1920, 1080));

    // The contained bounds carry the paint: sample the center of the scaled
    // bounds and expect the reddish source, not the black background.
    let percent = 100;
    let center_x = (u64::from(placed.bounds.x) * percent / 100) as u32 + placed.bounds.width / 2;
    let center_y = (u64::from(placed.bounds.y) * percent / 100) as u32 + placed.bounds.height / 2;
    let [r, g, b, a] = pixel(raster.rgba(), raster.width(), center_x, center_y);
    assert_eq!(a, 255);
    assert!(
        r > 100 && g < 100 && b < 100,
        "contained bounds must carry the resolved image, got [{r},{g},{b},{a}]"
    );
    // Outside the image the canvas is still background.
    assert_eq!(pixel(raster.rgba(), raster.width(), 0, 0), [0, 0, 0, 255]);
}

#[test]
fn missing_and_bad_images_fail_structurally() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let content = TemplateContent {
        kind: TemplateKind::GeneralImage,
        header: None,
        body: String::new(),
        footer: None,
        image: Some(ForegroundImage {
            id: "absent-asset".to_owned(),
            natural_width: 80,
            natural_height: 60,
        }),
    };
    let (frame, _) = render_template(&content, &theme, density).unwrap();

    assert_eq!(
        rasterize(&frame, density, &HashMap::new()),
        Err(RasterError::UnknownImage {
            id: "absent-asset".to_owned()
        })
    );

    let mut bad = HashMap::new();
    bad.insert("absent-asset".to_owned(), b"not a png".to_vec());
    assert_eq!(
        rasterize(&frame, density, &bad),
        Err(RasterError::InvalidImageBytes {
            id: "absent-asset".to_owned()
        })
    );
}

#[test]
fn png_encoding_round_trips_through_the_qualified_edge() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    let (frame, _) = render_template(&lyrics("Dayegon ang Ginoo!"), &theme, density).unwrap();
    let raster = rasterize(&frame, density, &HashMap::new()).unwrap();
    let encoded = raster.encode_png().expect("raster must encode as PNG");
    assert!(!encoded.is_empty());
    let decoded = tiny_skia::Pixmap::decode_png(&encoded).expect("our PNG must decode");
    assert_eq!(decoded.width(), raster.width());
    assert_eq!(decoded.height(), raster.height());
}
