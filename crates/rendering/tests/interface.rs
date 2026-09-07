//! T0.4 PR 3: the Renderer Interface accepts resolved content, a Theme, and a
//! target density, and returns either one complete frame plus diagnostics or
//! one structured failure. Every check below goes through [`rendering::render`],
//! never through its internals.

use rendering::{
    BODY_LINE_HEIGHT, LOGICAL_HEIGHT, LOGICAL_WIDTH, MIN_SAFE_INSET_X, MIN_SAFE_INSET_Y, Rect,
    RenderError, SemanticContent, TargetDensity, Theme, render,
};

fn body_only(body: &str) -> SemanticContent {
    SemanticContent {
        body: body.to_owned(),
        response_header: None,
        psalm_header: None,
        footer: None,
    }
}

#[test]
fn complete_frames_carry_the_whole_composition() {
    let (frame, diagnostics) = render(
        &body_only("Maayong buntag, mga igsoon!"),
        &Theme::default(),
        TargetDensity::SCALE_100,
    )
    .expect("body-only content with the minimum Theme must compose");

    assert_eq!(frame.logical_size(), (LOGICAL_WIDTH, LOGICAL_HEIGHT));
    assert_eq!(
        frame.safe_area(),
        Rect {
            x: 120,
            y: 80,
            width: 1680,
            height: 920,
        }
    );
    // Absent zones collapse: with neither header nor footer, the body is the
    // whole safe area, so there is no partial frame to mistake for complete.
    assert_eq!(frame.header(), None);
    assert_eq!(frame.footer(), None);
    assert_eq!(frame.body(), frame.safe_area());
    assert!(diagnostics.warnings.is_empty());
}

#[test]
fn absent_zones_return_their_space_to_the_body() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;
    // Measured zone heights (PR 4): one wrapped line at the provisional size.
    let line = BODY_LINE_HEIGHT;

    let (neither, _) = render(&body_only("body"), &theme, density).unwrap();
    let neither_body = neither.body();
    assert_eq!(neither_body.height, 920);

    let with_header = SemanticContent {
        response_header: Some("TUBAG:".to_owned()),
        ..body_only("body")
    };
    let (header_frame, _) = render(&with_header, &theme, density).unwrap();
    let header = header_frame
        .header()
        .expect("present header reserves a zone");
    assert_eq!(header.height, line);
    assert_eq!(header_frame.body().height, 920 - line);
    assert_eq!(header_frame.footer(), None);

    let with_footer = SemanticContent {
        footer: Some("Salmo 95".to_owned()),
        ..body_only("body")
    };
    let (footer_frame, _) = render(&with_footer, &theme, density).unwrap();
    assert_eq!(footer_frame.header(), None);
    assert_eq!(footer_frame.body().height, 920 - line);

    let with_both = SemanticContent {
        response_header: Some("TUBAG:".to_owned()),
        footer: Some("Salmo 95".to_owned()),
        ..body_only("body")
    };
    let (both_frame, _) = render(&with_both, &theme, density).unwrap();
    assert_eq!(both_frame.body().height, 920 - 2 * line);

    // Zones tile the safe area exactly: same width, stacked without gaps.
    for frame in [&header_frame, &footer_frame, &both_frame] {
        let safe = frame.safe_area();
        let body = frame.body();
        assert_eq!((body.x, body.width), (safe.x, safe.width));
        let header_bottom = frame.header().map_or(safe.y, |zone| zone.y + zone.height);
        assert_eq!(body.y, header_bottom);
        let footer_top = frame.footer().map_or(safe.y + safe.height, |zone| zone.y);
        assert_eq!(body.y + body.height, footer_top);
    }
}

#[test]
fn empty_string_header_reserves_while_absent_header_collapses() {
    let theme = Theme::default();
    let density = TargetDensity::SCALE_100;

    let (empty, _) = render(
        &SemanticContent {
            response_header: Some(String::new()),
            ..body_only("body")
        },
        &theme,
        density,
    )
    .unwrap();
    let (absent, _) = render(&body_only("body"), &theme, density).unwrap();

    assert!(empty.header().is_some());
    assert_eq!(absent.header(), None);
    assert!(empty.body().height < absent.body().height);
}

#[test]
fn theme_may_increase_but_not_reduce_the_safe_area() {
    let content = body_only("body");
    let density = TargetDensity::SCALE_100;

    let (grown, _) = render(
        &content,
        &Theme {
            safe_inset_x: 200,
            safe_inset_y: 120,
        },
        density,
    )
    .expect("a Theme may increase the safe area");
    assert_eq!(
        grown.safe_area(),
        Rect {
            x: 200,
            y: 120,
            width: 1520,
            height: 840,
        }
    );

    // Reductions fail with the requested values attached, proving the
    // renderer rejects rather than clamping silently.
    assert_eq!(
        render(
            &content,
            &Theme {
                safe_inset_x: 119,
                safe_inset_y: 80,
            },
            density
        ),
        Err(RenderError::SafeAreaBelowMinimum {
            requested_x: 119,
            requested_y: 80,
            minimum_x: 120,
            minimum_y: 80,
        })
    );
    assert_eq!(
        render(
            &content,
            &Theme {
                safe_inset_x: 120,
                safe_inset_y: 79,
            },
            density
        ),
        Err(RenderError::SafeAreaBelowMinimum {
            requested_x: 120,
            requested_y: 79,
            minimum_x: 120,
            minimum_y: 80,
        })
    );
}

#[test]
fn conflicting_headers_fail_instead_of_picking_silently() {
    assert_eq!(
        render(
            &SemanticContent {
                response_header: Some("TUBAG:".to_owned()),
                psalm_header: Some("SALMO RESPONSORYO".to_owned()),
                ..body_only("body")
            },
            &Theme::default(),
            TargetDensity::SCALE_100,
        ),
        Err(RenderError::ConflictingHeaders)
    );
}

#[test]
fn target_density_never_alters_the_logical_composition() {
    let content = SemanticContent {
        response_header: Some("TUBAG:".to_owned()),
        footer: Some("Salmo 95".to_owned()),
        ..body_only("Ang ginoo maanaa kaninyo")
    };
    let theme = Theme::default();

    assert!(TargetDensity::from_percent(0).is_none());
    let light = TargetDensity::from_percent(100).unwrap();
    let dense = TargetDensity::from_percent(200).unwrap();
    assert_eq!(dense.scale_percent(), 200);

    let (at_light, diagnostics_light) = render(&content, &theme, light).unwrap();
    let (at_dense, diagnostics_dense) = render(&content, &theme, dense).unwrap();
    assert_eq!(at_light, at_dense);
    assert_eq!(diagnostics_light, diagnostics_dense);
}

#[test]
fn logical_geometry_matches_the_design_contract() {
    assert_eq!((LOGICAL_WIDTH, LOGICAL_HEIGHT), (1920, 1080));
    assert_eq!((MIN_SAFE_INSET_X, MIN_SAFE_INSET_Y), (120, 80));
    const DESIGN_MD: &str = include_str!("../../../DESIGN.md");
    assert!(DESIGN_MD.contains("120px"), "safe-area prose moved");
    assert!(DESIGN_MD.contains("80px"), "safe-area prose moved");
    assert!(
        DESIGN_MD.contains("minimum default safe area"),
        "safe-area minimum wording moved"
    );
}

#[test]
fn interface_has_no_language_viewport_or_placement_expression() {
    // The exit check is structural: with no language type, no container or
    // scale parameter, and no placement parameter, neither a language lookup
    // nor a viewport-dependent composition nor free positioning can be written
    // against this Interface. The scan below keeps it that way.
    const FORBIDDEN: &[&str] = &[
        "TUBAG",
        "SALMO",
        "Language",
        "translate",
        "locale",
        "viewport",
        "Viewport",
        "InterfaceScale",
        "interface_scale",
        "system_scale",
        "device_scale",
        "DisplayScale",
        "draggable",
        "Draggable",
        "gpui",
        "fontdb",
        "DirectWrite",
        "fontconfig",
        "CoreText",
    ];
    let mut checked = 0;
    visit(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut checked,
    );
    assert!(checked > 0, "expected rendering sources to scan");

    fn visit(directory: &std::path::Path, checked: &mut usize) {
        for entry in std::fs::read_dir(directory).expect("rendering src must be readable") {
            let path = entry.expect("readable entry").path();
            if path.is_dir() {
                visit(&path, checked);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                *checked += 1;
                let source = std::fs::read_to_string(&path).expect("readable source");
                for token in FORBIDDEN {
                    assert!(
                        !source.contains(token),
                        "{} expresses {token:?}",
                        path.display()
                    );
                }
            }
        }
    }
}
