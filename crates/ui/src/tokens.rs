//! Typed values for the semantic design tokens in `design-tokens.json`.

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

/// `xtask verify` pins `design-tokens.json` to `DESIGN.md`; these assertions pin
/// the typed constants above to that same file, so no transcription can drift
/// silently. Leaf token names are unique across the document, which is what
/// makes the containment check unambiguous.
#[cfg(test)]
mod tests {
    use super::*;

    const TOKENS_JSON: &str = include_str!("../design-tokens.json");

    #[track_caller]
    fn assert_declares(entry: String) {
        assert!(
            TOKENS_JSON.contains(&entry),
            "design-tokens.json does not declare {entry}"
        );
    }

    fn assert_color(name: &str, value: u32) {
        assert_declares(format!("\"{name}\": \"#{value:06X}\""));
    }

    fn assert_scalar(name: &str, value: f32) {
        assert_declares(format!("\"{name}\": {}", value as u16));
    }

    fn assert_type_style(name: &str, style: TypeStyle) {
        assert_declares(format!(
            "\"{name}\": {{ \"size\": {}, \"weight\": {}, \"line_height\": {} }}",
            style.size as u16, style.weight, style.line_height
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
        assert_declares(format!(
            "\"operator_family\": \"{}\"",
            typography::OPERATOR_FAMILY
        ));
        assert_declares(format!(
            "\"fallback_family\": \"{}\"",
            typography::FALLBACK_FAMILY
        ));
        assert_type_style("title", typography::TITLE);
        assert_type_style("body", typography::BODY);
        assert_type_style("label", typography::LABEL);
        assert_type_style("projected_response", typography::PROJECTED_RESPONSE);

        assert_declares(format!("\"family\": \"{}\"", icon::FAMILY));
        assert_scalar("control_size", icon::CONTROL_SIZE);
        assert_scalar("navigation_size", icon::NAVIGATION_SIZE);
        assert_scalar("stroke_width", icon::STROKE_WIDTH);
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
            ("separator", border::SEPARATOR),
            ("focus_separation", border::FOCUS_SEPARATION),
            ("focus_outer", border::FOCUS_OUTER),
            ("compact_control", density::COMPACT_CONTROL),
            ("default_control", density::DEFAULT_CONTROL),
            ("presenter_control", density::PRESENTER_CONTROL),
            (
                "workspace_gutter_compact",
                density::WORKSPACE_GUTTER_COMPACT,
            ),
            ("workspace_gutter_wide", density::WORKSPACE_GUTTER_WIDE),
        ] {
            assert_scalar(name, value);
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
            assert_declares(format!("\"{name}\": \"{value}\""));
        }

        for (name, value) in [
            ("quick_duration_ms", motion::QUICK_DURATION_MS),
            ("standard_duration_ms", motion::STANDARD_DURATION_MS),
            ("overlay_open_duration_ms", motion::OVERLAY_OPEN_DURATION_MS),
            (
                "overlay_close_duration_ms",
                motion::OVERLAY_CLOSE_DURATION_MS,
            ),
            (
                "output_content_duration_ms",
                motion::OUTPUT_CONTENT_DURATION_MS,
            ),
            ("output_blank_duration_ms", motion::OUTPUT_BLANK_DURATION_MS),
            (
                "reduced_motion_max_duration_ms",
                motion::REDUCED_MOTION_MAX_DURATION_MS,
            ),
        ] {
            assert_declares(format!("\"{name}\": {value}"));
        }

        assert_declares(format!(
            "\"interface_scales\": [{}]",
            INTERFACE_SCALES
                .map(|scale| scale.to_string())
                .join(", ")
                .as_str()
        ));
    }
}
