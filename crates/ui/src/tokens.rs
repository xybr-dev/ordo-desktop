//! Typed values for the semantic design tokens defined by `DESIGN.md`.
//!
//! `DESIGN.md`'s YAML frontmatter is the normative source. These constants are
//! the only machine-readable copy; the tests below pin every one of them back
//! to that frontmatter so the transcription cannot drift silently.

use crate::environment::ResolvedAppearance;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeStyle {
    pub size: f32,
    pub weight: u16,
    pub line_height: f32,
}

pub mod color {
    pub const PREPARATION_BACKGROUND: u32 = 0xF4F0E7;
    pub const PREPARATION_SURFACE: u32 = 0xFFFDF8;
    pub const PREPARATION_SURFACE_MUTED: u32 = 0xEBE5DB;
    pub const PREPARATION_TEXT: u32 = 0x27231F;
    pub const PREPARATION_TEXT_MUTED: u32 = 0x716B62;
    pub const PREPARATION_BORDER: u32 = 0xD7CFC2;
    pub const PREPARATION_BORDER_STRONG: u32 = 0x8E8476;
    pub const PRESENTER_BACKGROUND: u32 = 0x151816;
    pub const PRESENTER_SURFACE: u32 = 0x222722;
    pub const PRESENTER_SURFACE_RAISED: u32 = 0x303630;
    pub const PRESENTER_TEXT: u32 = 0xF2EFE8;
    pub const PRESENTER_TEXT_MUTED: u32 = 0xB1ADA4;
    pub const PRESENTER_BORDER: u32 = 0x414842;
    pub const PRESENTER_BORDER_STRONG: u32 = 0x7A857B;
    pub const PRIMARY: u32 = 0x355D4B;
    pub const ON_PRIMARY: u32 = 0xFFFFFF;
    pub const SELECTION_PREPARATION: u32 = 0xDFE9E2;
    pub const SELECTION_PRESENTER: u32 = 0x26362C;
    pub const LIVE_PREPARATION: u32 = 0x2F6B4F;
    pub const LIVE_PRESENTER: u32 = 0x55D69C;
    pub const FREEZE_PREPARATION: u32 = 0x9A5B10;
    pub const FREEZE_PRESENTER: u32 = 0xF0B35E;
    pub const BLACKOUT: u32 = 0x050606;
    pub const ON_BLACKOUT: u32 = 0xFFFFFF;
    pub const WARNING_PREPARATION: u32 = 0x9A5B10;
    pub const WARNING_PRESENTER: u32 = 0xF0B35E;
    pub const DANGER_PREPARATION: u32 = 0x9D3E31;
    pub const DANGER_PRESENTER: u32 = 0xEF8E82;
    pub const INFORMATION_PREPARATION: u32 = 0x315E83;
    pub const INFORMATION_PRESENTER: u32 = 0x9ACDF2;
    pub const FOCUS_PREPARATION: u32 = 0x2F6FA3;
    pub const FOCUS_PRESENTER: u32 = 0x8FCBFF;
    pub const PROJECTION_BACKGROUND: u32 = 0x000000;
    pub const PROJECTION_TEXT: u32 = 0xFFFFFF;
    pub const LITURGICAL_PURPLE: u32 = 0x6D4A87;
    pub const LITURGICAL_RED: u32 = 0xA43F38;
    pub const LITURGICAL_GREEN: u32 = 0x46744D;
    pub const LITURGICAL_ROSE: u32 = 0xC9788E;
    pub const LITURGICAL_WHITE: u32 = 0xF9F5E9;
}

/// The appearance-resolved half of [`color`].
///
/// `DESIGN.md` pairs every operator color as `*-preparation`/`*-presenter`;
/// Light resolves to the Preparation value and Dark to the Presenter one. One
/// resolver keeps that match out of every surface that needs a color.
///
/// Live, Freeze, Blackout, and the liturgical colors are deliberately absent:
/// they are output and content state, not operator chrome, and arrive with the
/// surfaces that own them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Palette {
    pub background: u32,
    pub surface: u32,
    /// Preparation's muted fill and Presenter's raised fill: the same role, in
    /// the direction each appearance can move away from its surface.
    pub surface_secondary: u32,
    pub text: u32,
    pub text_muted: u32,
    pub border: u32,
    pub border_strong: u32,
    pub selection: u32,
    pub focus: u32,
    pub warning: u32,
    pub danger: u32,
    pub information: u32,
}

pub const fn palette(appearance: ResolvedAppearance) -> Palette {
    match appearance {
        ResolvedAppearance::Light => Palette {
            background: color::PREPARATION_BACKGROUND,
            surface: color::PREPARATION_SURFACE,
            surface_secondary: color::PREPARATION_SURFACE_MUTED,
            text: color::PREPARATION_TEXT,
            text_muted: color::PREPARATION_TEXT_MUTED,
            border: color::PREPARATION_BORDER,
            border_strong: color::PREPARATION_BORDER_STRONG,
            selection: color::SELECTION_PREPARATION,
            focus: color::FOCUS_PREPARATION,
            warning: color::WARNING_PREPARATION,
            danger: color::DANGER_PREPARATION,
            information: color::INFORMATION_PREPARATION,
        },
        ResolvedAppearance::Dark => Palette {
            background: color::PRESENTER_BACKGROUND,
            surface: color::PRESENTER_SURFACE,
            surface_secondary: color::PRESENTER_SURFACE_RAISED,
            text: color::PRESENTER_TEXT,
            text_muted: color::PRESENTER_TEXT_MUTED,
            border: color::PRESENTER_BORDER,
            border_strong: color::PRESENTER_BORDER_STRONG,
            selection: color::SELECTION_PRESENTER,
            focus: color::FOCUS_PRESENTER,
            warning: color::WARNING_PRESENTER,
            danger: color::DANGER_PRESENTER,
            information: color::INFORMATION_PRESENTER,
        },
    }
}

pub mod typography {
    use super::TypeStyle;

    pub const OPERATOR_FAMILY: &str = "Atkinson Hyperlegible Next";
    pub const FALLBACK_FAMILY: &str = "Noto Sans";
    pub const TITLE: TypeStyle = TypeStyle {
        size: 24.0,
        weight: 600,
        line_height: 1.2,
    };
    pub const BODY: TypeStyle = TypeStyle {
        size: 16.0,
        weight: 400,
        line_height: 1.45,
    };
    pub const LABEL: TypeStyle = TypeStyle {
        size: 14.0,
        weight: 500,
        line_height: 1.3,
    };
    pub const PROJECTED_RESPONSE: TypeStyle = TypeStyle {
        size: 72.0,
        weight: 800,
        line_height: 1.15,
    };
}

pub mod spacing {
    pub const UNIT: f32 = 4.0;
    pub const RELATED_TIGHT: f32 = 4.0;
    pub const RELATED: f32 = 8.0;
    pub const CONTROL_INSET: f32 = 12.0;
    pub const GROUP: f32 = 16.0;
    pub const SECTION: f32 = 24.0;
}

pub mod radius {
    pub const CONTROL: f32 = 4.0;
    pub const OVERLAY: f32 = 8.0;
    pub const SLIDE_VIEWPORT: f32 = 4.0;
}

pub mod border {
    pub const SEPARATOR: f32 = 1.0;
    pub const FOCUS_SEPARATION: f32 = 2.0;
    pub const FOCUS_OUTER: f32 = 2.0;
}

pub mod elevation {
    pub const FLOATING_PREPARATION: &str = "0 4px 12px rgba(39, 35, 31, 0.14)";
    pub const OVERLAY_PREPARATION: &str = "0 18px 48px rgba(39, 35, 31, 0.20)";
    pub const FLOATING_PRESENTER: &str = "0 4px 14px rgba(0, 0, 0, 0.40)";
    pub const OVERLAY_PRESENTER: &str = "0 18px 52px rgba(0, 0, 0, 0.58)";
}

pub mod icon {
    pub const FAMILY: &str = "Lucide";
    pub const CONTROL_SIZE: f32 = 16.0;
    pub const NAVIGATION_SIZE: f32 = 20.0;
    pub const STROKE_WIDTH: f32 = 2.0;
}

pub mod density {
    pub const COMPACT_CONTROL: f32 = 32.0;
    pub const DEFAULT_CONTROL: f32 = 36.0;
    pub const PRESENTER_CONTROL: f32 = 40.0;
    pub const WORKSPACE_GUTTER_COMPACT: f32 = 16.0;
    pub const WORKSPACE_GUTTER_WIDE: f32 = 24.0;
}

pub mod motion {
    pub const QUICK_DURATION_MS: u16 = 120;
    pub const STANDARD_DURATION_MS: u16 = 160;
    pub const OVERLAY_OPEN_DURATION_MS: u16 = 400;
    pub const OVERLAY_CLOSE_DURATION_MS: u16 = 200;
    pub const OUTPUT_CONTENT_DURATION_MS: u16 = 0;
    pub const OUTPUT_BLANK_DURATION_MS: u16 = 500;
    pub const REDUCED_MOTION_MAX_DURATION_MS: u16 = 100;
    pub const EASING: &str = "cubic-bezier(0.2, 0, 0, 1)";
}

pub const INTERFACE_SCALES: [u16; 5] = [100, 125, 150, 175, 200];

/// The invariant logical canvas every Slide composes in, independent of GPUI
/// viewport size, Interface scale, OS scale, and physical output density.
pub mod canonical_slide {
    pub const WIDTH: u32 = 1920;
    pub const HEIGHT: u32 = 1080;
    pub const ASPECT_RATIO: &str = "16:9";
}

/// Pins every constant above to `DESIGN.md`'s normative YAML frontmatter, so a
/// token edited in the design contract but not here fails the build.
///
/// Entries are matched with their leading newline and two-space indent, which
/// defeats the suffix case (`control` inside `compact-control`) but does not
/// bind a token to its own top-level map, since every one of these sits at the
/// same indent. Two sections sharing a leaf name would let one satisfy the
/// other's assertion; none do today, so check for it when adding a token.
///
/// ponytail: `borders`, `interface_scales`, `reduced_motion_max_duration_ms`,
/// and `fallback_family` live only in DESIGN.md's prose, not its frontmatter,
/// so they have no automated check. Promote them to the frontmatter to cover
/// them; matching prose instead would break on ordinary wording edits.
#[cfg(test)]
mod tests {
    use super::*;

    const DESIGN_MD: &str = include_str!("../../../DESIGN.md");

    /// The frontmatter alone, so prose that happens to quote a token value
    /// cannot satisfy an assertion.
    fn frontmatter() -> &'static str {
        DESIGN_MD
            .strip_prefix("---\n")
            .and_then(|rest| rest.split_once("\n---\n"))
            .expect("DESIGN.md must open with YAML frontmatter")
            .0
    }

    #[track_caller]
    fn assert_declares(entry: String) {
        assert!(
            frontmatter().contains(&entry),
            "DESIGN.md frontmatter does not declare{entry}"
        );
    }

    /// Rust constants are grouped under snake_case names; the frontmatter spells
    /// the same tokens in kebab-case.
    #[track_caller]
    fn assert_entry(name: &str, value: String) {
        assert_declares(format!("\n  {}: {value}", name.replace('_', "-")));
    }

    fn assert_color(name: &str, value: u32) {
        assert_entry(name, format!("\"#{value:06X}\""));
    }

    /// Frontmatter geometry carries a `px` suffix; durations carry `ms`.
    ///
    /// The cast to `u16` would silently truncate, letting a hairline `1.5`
    /// satisfy a frontmatter `1px`, so a fractional constant is rejected rather
    /// than compared. DESIGN.md has no fractional geometry; if it ever gains
    /// one, this must format the fraction instead of loosening the guard.
    #[track_caller]
    fn assert_scalar(name: &str, value: f32, unit: &str) {
        assert_eq!(
            value.fract(),
            0.0,
            "{name} is fractional and cannot be compared as an integer"
        );
        assert_entry(name, format!("{}{unit}", value as u16));
    }

    /// Type levels nest under their name, so the whole block is matched at once
    /// and each `fontSize` stays bound to the level that declares it.
    #[track_caller]
    fn assert_type_style(name: &str, style: TypeStyle) {
        assert_eq!(
            style.size.fract(),
            0.0,
            "{name} fontSize is fractional and cannot be compared as an integer"
        );
        assert_declares(format!(
            "\n  {}:\n    fontFamily: {}\n    fontSize: {}px\n    fontWeight: {}\n    lineHeight: {}",
            name.replace('_', "-"),
            typography::OPERATOR_FAMILY,
            style.size as u16,
            style.weight,
            style.line_height
        ));
    }

    #[test]
    fn colors_match_design_tokens() {
        use color::*;

        for (name, value) in [
            ("preparation_background", PREPARATION_BACKGROUND),
            ("preparation_surface", PREPARATION_SURFACE),
            ("preparation_surface_muted", PREPARATION_SURFACE_MUTED),
            ("preparation_text", PREPARATION_TEXT),
            ("preparation_text_muted", PREPARATION_TEXT_MUTED),
            ("preparation_border", PREPARATION_BORDER),
            ("preparation_border_strong", PREPARATION_BORDER_STRONG),
            ("presenter_background", PRESENTER_BACKGROUND),
            ("presenter_surface", PRESENTER_SURFACE),
            ("presenter_surface_raised", PRESENTER_SURFACE_RAISED),
            ("presenter_text", PRESENTER_TEXT),
            ("presenter_text_muted", PRESENTER_TEXT_MUTED),
            ("presenter_border", PRESENTER_BORDER),
            ("presenter_border_strong", PRESENTER_BORDER_STRONG),
            ("primary", PRIMARY),
            ("on_primary", ON_PRIMARY),
            ("selection_preparation", SELECTION_PREPARATION),
            ("selection_presenter", SELECTION_PRESENTER),
            ("live_preparation", LIVE_PREPARATION),
            ("live_presenter", LIVE_PRESENTER),
            ("freeze_preparation", FREEZE_PREPARATION),
            ("freeze_presenter", FREEZE_PRESENTER),
            ("blackout", BLACKOUT),
            ("on_blackout", ON_BLACKOUT),
            ("warning_preparation", WARNING_PREPARATION),
            ("warning_presenter", WARNING_PRESENTER),
            ("danger_preparation", DANGER_PREPARATION),
            ("danger_presenter", DANGER_PRESENTER),
            ("information_preparation", INFORMATION_PREPARATION),
            ("information_presenter", INFORMATION_PRESENTER),
            ("focus_preparation", FOCUS_PREPARATION),
            ("focus_presenter", FOCUS_PRESENTER),
            ("projection_background", PROJECTION_BACKGROUND),
            ("projection_text", PROJECTION_TEXT),
            ("liturgical_purple", LITURGICAL_PURPLE),
            ("liturgical_red", LITURGICAL_RED),
            ("liturgical_green", LITURGICAL_GREEN),
            ("liturgical_rose", LITURGICAL_ROSE),
            ("liturgical_white", LITURGICAL_WHITE),
        ] {
            assert_color(name, value);
        }
    }

    #[test]
    fn typography_and_icons_match_design_tokens() {
        assert_type_style("operator_title", typography::TITLE);
        assert_type_style("operator_body", typography::BODY);
        assert_type_style("operator_label", typography::LABEL);
        assert_type_style("projected_response", typography::PROJECTED_RESPONSE);

        assert_entry("family", icon::FAMILY.to_owned());
        assert_scalar("control_size", icon::CONTROL_SIZE, "px");
        assert_scalar("navigation_size", icon::NAVIGATION_SIZE, "px");
        assert_scalar("stroke_width", icon::STROKE_WIDTH, "px");
    }

    #[test]
    fn geometry_matches_design_tokens() {
        for (name, value) in [
            ("unit", spacing::UNIT),
            ("related_tight", spacing::RELATED_TIGHT),
            ("related", spacing::RELATED),
            ("control_inset", spacing::CONTROL_INSET),
            ("group", spacing::GROUP),
            ("section", spacing::SECTION),
            ("control", radius::CONTROL),
            ("overlay", radius::OVERLAY),
            ("slide_viewport", radius::SLIDE_VIEWPORT),
            ("compact_control", density::COMPACT_CONTROL),
            ("default_control", density::DEFAULT_CONTROL),
            ("presenter_control", density::PRESENTER_CONTROL),
            (
                "workspace_gutter_compact",
                density::WORKSPACE_GUTTER_COMPACT,
            ),
            ("workspace_gutter_wide", density::WORKSPACE_GUTTER_WIDE),
        ] {
            assert_scalar(name, value, "px");
        }
    }

    #[test]
    fn elevation_and_motion_match_design_tokens() {
        for (name, value) in [
            ("floating_preparation", elevation::FLOATING_PREPARATION),
            ("overlay_preparation", elevation::OVERLAY_PREPARATION),
            ("floating_presenter", elevation::FLOATING_PRESENTER),
            ("overlay_presenter", elevation::OVERLAY_PRESENTER),
            ("easing", motion::EASING),
        ] {
            assert_entry(name, format!("\"{value}\""));
        }

        for (name, value) in [
            ("quick_duration", motion::QUICK_DURATION_MS),
            ("standard_duration", motion::STANDARD_DURATION_MS),
            ("overlay_open_duration", motion::OVERLAY_OPEN_DURATION_MS),
            ("overlay_close_duration", motion::OVERLAY_CLOSE_DURATION_MS),
            (
                "output_content_duration",
                motion::OUTPUT_CONTENT_DURATION_MS,
            ),
            ("output_blank_duration", motion::OUTPUT_BLANK_DURATION_MS),
        ] {
            assert_scalar(name, f32::from(value), "ms");
        }
    }

    /// The one invariant `xtask` used to guard through the deleted intermediate.
    /// DESIGN.md states it in prose rather than frontmatter, but the literal
    /// geometry is the contract itself, not incidental wording.
    #[test]
    fn canonical_slide_geometry_is_stated_by_design_md() {
        assert_eq!(canonical_slide::WIDTH, 1920);
        assert_eq!(canonical_slide::HEIGHT, 1080);
        assert_eq!(canonical_slide::ASPECT_RATIO, "16:9");
        assert!(DESIGN_MD.contains("`1920x1080` logical canvas"));
        assert!(DESIGN_MD.contains("canonical `16:9` geometry"));
    }
}
