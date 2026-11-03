//! Pure Presenter layout model.
//!
//! Given the window size, Interface scale and the operator's rail choice, this
//! decides the regime, rail, Output Dock and Cue grid. It uses plain `f32`
//! logical pixels, so it is unit-testable without a window and the view only
//! has to apply the result. It mirrors `layout()`, `railPinned()` and
//! `maxCols()` in `docs/mockups/ordo-kit.js` (about lines 296-322) and the
//! monitor sizing in `render()` (about lines 868-897), per DESIGN.md
//! "Constrained Presenter Layouts" and "Interface Scaling and Slide
//! Isolation".
//!
//! Invariants: every width and height returned is in window pixels (already
//! multiplied by the Interface scale), the Output Dock controls never shrink,
//! and a Cue frame never drops below the 270 px floor (scaled) when more than
//! one column is shown.

use ui::environment::InterfaceScale;

/// Effective width (`width / scale`) at or above which the regime is Wide.
/// Kit `layout()`: `eff >= 1500`.
pub const WIDE_MIN_EFFECTIVE_WIDTH: f32 = 1500.0;
/// Effective width at or above which the regime is Compact. Kit `layout()`.
pub const COMPACT_MIN_EFFECTIVE_WIDTH: f32 = 1000.0;

/// Pinned Ordo rail width (kit `.pr-rail`, `200 * s`).
pub const RAIL_PINNED_WIDTH: f32 = 200.0;
/// Width of the collapsed ORDO tab (kit `maxCols()`, `28 * s`).
pub const RAIL_TAB_WIDTH: f32 = 28.0;

/// Output Dock width at Wide (Decision 296, kit `.pr-dock`).
pub const DOCK_WIDE_WIDTH: f32 = 520.0;
/// Output Dock width at Compact (kit `[data-layout="compact"] .pr-dock`).
pub const DOCK_COMPACT_WIDTH: f32 = 360.0;
/// Dock padding on each side (kit `--o-s4`; spec: dock inner = dock - 2 * 16).
pub const DOCK_PADDING: f32 = 16.0;
/// Vertical gap between dock items (kit `--o-s3`).
pub const DOCK_GAP: f32 = 12.0;
/// Gap between a monitor's label row and its 16:9 slide (kit `--o-s2`).
pub const MONITOR_LABEL_GAP: f32 = 8.0;
/// Presenter header height (kit `.pr-header`, `min-height: 48px`).
pub const HEADER_HEIGHT: f32 = 48.0;
/// Output status row below the monitors (spec: 32).
pub const DOCK_STATUS_HEIGHT: f32 = 32.0;
/// Freeze / Blackout row (spec: 40).
pub const DOCK_FREEZE_BLACKOUT_HEIGHT: f32 = 40.0;
/// Primary action button (spec: 48; kit `--o-h-presenter`).
pub const DOCK_PRIMARY_HEIGHT: f32 = 48.0;
/// Label row above each monitor (kit `.pr-mon-head`: 14 px text, rounded up
/// to 24 to leave room for the state glyph). An approximation of the kit's
/// DOM-measured overflow, not a kit token.
pub const MONITOR_LABEL_HEIGHT: f32 = 24.0;
/// Items stacked in the dock: two monitors, status, Freeze/Blackout, primary.
const DOCK_ITEM_COUNT: f32 = 5.0;
/// Monitors stacked in the Side dock (Program and Preview).
const DOCK_MONITOR_COUNT: f32 = 2.0;

/// Controls that never shrink: status, Freeze/Blackout and the primary action.
pub const DOCK_CONTROLS_HEIGHT: f32 =
    DOCK_STATUS_HEIGHT + DOCK_FREEZE_BLACKOUT_HEIGHT + DOCK_PRIMARY_HEIGHT;

/// Height the Side dock reserves for everything except the two 16:9 slides:
/// header, both label rows with their gaps, the never-shrinking controls, the
/// gaps between the five dock items and the dock's own top and bottom padding.
/// The kit measures this overflow in the DOM (`dockEl.scrollHeight`); this
/// sum approximates it and is not a kit token.
pub const DOCK_RESERVED_HEIGHT: f32 = HEADER_HEIGHT
    + DOCK_MONITOR_COUNT * (MONITOR_LABEL_HEIGHT + MONITOR_LABEL_GAP)
    + DOCK_CONTROLS_HEIGHT
    + (DOCK_ITEM_COUNT - 1.0) * DOCK_GAP
    + 2.0 * DOCK_PADDING;

/// Band height as a share of window height (kit `H * 0.3`, not scaled).
pub const BAND_HEIGHT_SHARE: f32 = 0.3;
/// Band horizontal padding on each side (kit `.pr-band`, `--o-s4`).
pub const BAND_GUTTER: f32 = 16.0;
/// Gap between the Program and Preview monitors in the band (kit `--o-s3`).
pub const BAND_GAP: f32 = 12.0;

/// Frame width floor for a Cue (kit `maxCols()`, 270 px, scaled).
pub const FRAME_FLOOR_WIDTH: f32 = 270.0;
/// Cue padding on both sides of the frame (kit: 2 px each side).
pub const CELL_PADDING: f32 = 4.0;
/// Gap between Cue cells (kit `maxCols()`, `12 * s`).
pub const CELL_GAP: f32 = 12.0;
/// Timeline padding plus group indent (kit `(32 + 16) * s`).
pub const TIMELINE_INSET: f32 = 48.0;
/// Timeline scrollbar width. Unscaled (kit `maxCols()` subtracts a bare 15).
pub const SCROLLBAR_WIDTH: f32 = 15.0;
/// Column cap at Wide.
pub const WIDE_MAX_COLUMNS: u32 = 4;
/// Column cap at Compact and Constrained.
pub const NARROW_MAX_COLUMNS: u32 = 3;

/// 16:9 slide aspect used for monitor widths.
const SLIDE_ASPECT: f32 = 16.0 / 9.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Regime {
    Wide,
    Compact,
    Constrained,
}

/// The operator's rail choice for this session (Decision 237). `Auto` follows
/// the regime. Constrained is always overlay-only whatever the choice.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RailChoice {
    #[default]
    Auto,
    Pinned,
    Unpinned,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RailLayout {
    Pinned { width: f32 },
    Tab { width: f32 },
    OverlayOnly { tab_width: f32 },
}

impl RailLayout {
    /// Width the rail or its tab takes from the row.
    pub fn width(self) -> f32 {
        match self {
            Self::Pinned { width } | Self::Tab { width } => width,
            Self::OverlayOnly { tab_width } => tab_width,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DockLayout {
    Side { width: f32, monitor_width: f32 },
    Band { monitor_width: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutInput {
    pub width: f32,
    pub height: f32,
    pub scale: InterfaceScale,
    pub rail: RailChoice,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresenterLayout {
    pub regime: Regime,
    pub rail: RailLayout,
    pub dock: DockLayout,
    pub grid_columns: u32,
    /// Cue frame width (the 16:9 slide, excluding cell padding).
    pub cell_width: f32,
    /// Width available to the Cue timeline content.
    pub timeline_width: f32,
}

fn regime_for(width: f32, scale_factor: f32) -> Regime {
    let effective = width / scale_factor;
    if effective >= WIDE_MIN_EFFECTIVE_WIDTH {
        Regime::Wide
    } else if effective >= COMPACT_MIN_EFFECTIVE_WIDTH {
        Regime::Compact
    } else {
        Regime::Constrained
    }
}

pub fn presenter_layout(input: LayoutInput) -> PresenterLayout {
    let s = input.scale.apply(1.0);
    let width = input.width.max(0.0);
    let height = input.height.max(0.0);
    let regime = regime_for(width, s);

    let rail = match (regime, input.rail) {
        (Regime::Constrained, _) => RailLayout::OverlayOnly {
            tab_width: RAIL_TAB_WIDTH * s,
        },
        (_, RailChoice::Pinned) | (Regime::Wide, RailChoice::Auto) => RailLayout::Pinned {
            width: RAIL_PINNED_WIDTH * s,
        },
        _ => RailLayout::Tab {
            width: RAIL_TAB_WIDTH * s,
        },
    };

    let dock = match regime {
        Regime::Constrained => {
            let by_height = (height * BAND_HEIGHT_SHARE * SLIDE_ASPECT).floor();
            // The tab column sits beside the band, so it comes off the width first.
            let tab = RAIL_TAB_WIDTH * s;
            let by_width = (width - tab - 2.0 * BAND_GUTTER * s - BAND_GAP * s) / 2.0;
            DockLayout::Band {
                monitor_width: by_height.min(by_width).max(0.0),
            }
        }
        Regime::Wide | Regime::Compact => {
            let dock_width = if regime == Regime::Wide {
                DOCK_WIDE_WIDTH
            } else {
                DOCK_COMPACT_WIDTH
            } * s;
            let inner = dock_width - 2.0 * DOCK_PADDING * s;
            let by_height = (height - DOCK_RESERVED_HEIGHT * s) / DOCK_MONITOR_COUNT * SLIDE_ASPECT;
            DockLayout::Side {
                width: dock_width,
                monitor_width: inner.min(by_height).max(0.0),
            }
        }
    };

    let dock_taken = match dock {
        DockLayout::Side { width, .. } => width,
        DockLayout::Band { .. } => 0.0,
    };
    let timeline_width =
        (width - rail.width() - dock_taken - TIMELINE_INSET * s - SCROLLBAR_WIDTH).max(0.0);
    let max_columns = if regime == Regime::Wide {
        WIDE_MAX_COLUMNS
    } else {
        NARROW_MAX_COLUMNS
    };
    let per_cell = (FRAME_FLOOR_WIDTH + CELL_PADDING + CELL_GAP) * s;
    let fitting = ((timeline_width + CELL_GAP * s) / per_cell).floor();
    let grid_columns = (fitting as u32).clamp(1, max_columns);
    let cell_width = ((timeline_width + CELL_GAP * s) / grid_columns as f32
        - (CELL_GAP + CELL_PADDING) * s)
        .max(0.0);

    PresenterLayout {
        regime,
        rail,
        dock,
        grid_columns,
        cell_width,
        timeline_width,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCALES: [InterfaceScale; 5] = [
        InterfaceScale::Percent100,
        InterfaceScale::Percent125,
        InterfaceScale::Percent150,
        InterfaceScale::Percent175,
        InterfaceScale::Percent200,
    ];

    const EPS: f32 = 0.01;

    fn assert_close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < EPS, "{actual} != {expected}");
    }

    fn at(width: f32, height: f32, scale: InterfaceScale) -> PresenterLayout {
        presenter_layout(LayoutInput {
            width,
            height,
            scale,
            rail: RailChoice::Auto,
        })
    }

    #[test]
    fn wide_1920_by_1080_pins_the_rail_and_shows_four_columns() {
        let layout = at(1920.0, 1080.0, InterfaceScale::Percent100);
        assert_eq!(layout.regime, Regime::Wide);
        assert_eq!(layout.rail, RailLayout::Pinned { width: 200.0 });
        assert!(matches!(layout.dock, DockLayout::Side { width, .. } if width == 520.0));
        assert_eq!(layout.grid_columns, 4);
        assert_close(layout.timeline_width, 1137.0);
        assert_close(layout.cell_width, 271.25);
        assert!(
            matches!(layout.dock, DockLayout::Side { monitor_width, .. } if (monitor_width - 488.0).abs() < EPS)
        );
    }

    #[test]
    fn compact_1366_by_768_uses_the_tab_and_three_columns() {
        let layout = at(1366.0, 768.0, InterfaceScale::Percent100);
        assert_eq!(layout.regime, Regime::Compact);
        assert_eq!(layout.rail, RailLayout::Tab { width: 28.0 });
        assert!(matches!(layout.dock, DockLayout::Side { width, .. } if width == 360.0));
        assert_eq!(layout.grid_columns, 3);
        assert_close(layout.timeline_width, 915.0);
        assert_close(layout.cell_width, 293.0);
        assert!(
            matches!(layout.dock, DockLayout::Side { monitor_width, .. } if (monitor_width - 328.0).abs() < EPS)
        );
    }

    #[test]
    fn constrained_1366_by_768_at_200_percent_is_overlay_only_with_a_band() {
        let layout = at(1366.0, 768.0, InterfaceScale::Percent200);
        assert_eq!(layout.regime, Regime::Constrained);
        assert_eq!(layout.rail, RailLayout::OverlayOnly { tab_width: 56.0 });
        assert_eq!(
            layout.dock,
            DockLayout::Band {
                monitor_width: 409.0
            }
        );
        assert_eq!(layout.grid_columns, 2);
        assert_close(layout.timeline_width, 1199.0);
        assert_close(layout.cell_width, 579.5);
    }

    #[test]
    fn frames_keep_the_scaled_floor_and_side_controls_fit_at_every_scale() {
        for (width, height) in [(1920.0, 1080.0), (1366.0, 768.0)] {
            for scale in SCALES {
                let s = scale.apply(1.0);
                let layout = at(width, height, scale);
                if layout.grid_columns > 1 {
                    assert!(
                        layout.cell_width >= FRAME_FLOOR_WIDTH * s - 0.01,
                        "{width}x{height}@{}: {layout:?}",
                        scale.percent()
                    );
                }
                if let DockLayout::Side { monitor_width, .. } = layout.dock {
                    let used = DOCK_RESERVED_HEIGHT * s + 2.0 * monitor_width / SLIDE_ASPECT;
                    assert!(
                        used <= height + 0.01,
                        "{width}x{height}@{}",
                        scale.percent()
                    );
                }
            }
        }
    }

    #[test]
    fn a_short_window_shrinks_the_monitors_but_not_the_controls() {
        let tall = at(1366.0, 768.0, InterfaceScale::Percent100);
        let short = at(1366.0, 600.0, InterfaceScale::Percent100);
        let monitor = |layout: PresenterLayout| match layout.dock {
            DockLayout::Side { monitor_width, .. } => monitor_width,
            DockLayout::Band { .. } => panic!("expected a side dock"),
        };
        assert!(monitor(short) < monitor(tall));
        let used = DOCK_RESERVED_HEIGHT + 2.0 * monitor(short) / SLIDE_ASPECT;
        assert!(
            (used - 600.0).abs() < 0.01,
            "controls keep their full height"
        );
    }

    #[test]
    fn pinned_choice_is_ignored_when_constrained() {
        let layout = presenter_layout(LayoutInput {
            width: 900.0,
            height: 700.0,
            scale: InterfaceScale::Percent100,
            rail: RailChoice::Pinned,
        });
        assert!(matches!(layout.rail, RailLayout::OverlayOnly { .. }));
    }

    #[test]
    fn session_rail_choice_overrides_the_regime_default() {
        let choose = |width, rail| {
            presenter_layout(LayoutInput {
                width,
                height: 1080.0,
                scale: InterfaceScale::Percent100,
                rail,
            })
            .rail
        };
        assert_eq!(
            choose(1920.0, RailChoice::Unpinned),
            RailLayout::Tab { width: 28.0 }
        );
        assert_eq!(
            choose(1366.0, RailChoice::Pinned),
            RailLayout::Pinned { width: 200.0 }
        );
    }

    #[test]
    fn shrinking_the_window_never_adds_columns_or_breaks_the_floor() {
        let mut columns = Vec::new();
        let mut previous = u32::MAX;
        for width in [1920.0, 1500.0, 1200.0] {
            let layout = at(width, 900.0, InterfaceScale::Percent100);
            assert!(layout.grid_columns <= previous, "width {width}");
            if layout.grid_columns > 1 {
                assert!(layout.cell_width >= FRAME_FLOOR_WIDTH - 0.01);
            }
            previous = layout.grid_columns;
            columns.push(layout.grid_columns);
        }
        assert_eq!(columns, [4, 2, 2]);
    }

    #[test]
    fn band_monitors_fit_the_window_width() {
        let layout = at(900.0, 2000.0, InterfaceScale::Percent100);
        let DockLayout::Band { monitor_width } = layout.dock else {
            panic!("expected a band");
        };
        assert!(
            2.0 * monitor_width + BAND_GAP + 2.0 * BAND_GUTTER + RAIL_TAB_WIDTH <= 900.0 + 0.01
        );
    }

    #[test]
    fn band_and_tab_fit_the_width_at_every_scale() {
        for scale in [
            InterfaceScale::Percent100,
            InterfaceScale::Percent125,
            InterfaceScale::Percent150,
            InterfaceScale::Percent175,
            InterfaceScale::Percent200,
        ] {
            let s = scale.apply(1.0);
            for width in [700.0, 900.0, 1366.0] {
                let layout = at(width, 2000.0, scale);
                let DockLayout::Band { monitor_width } = layout.dock else {
                    continue;
                };
                let used =
                    2.0 * monitor_width + 2.0 * BAND_GUTTER * s + BAND_GAP * s + RAIL_TAB_WIDTH * s;
                assert!(used <= width + EPS, "{width} at {scale:?}: {used}");
            }
        }
    }

    #[test]
    fn band_monitors_shrink_to_the_width_at_200_percent() {
        let layout = at(900.0, 2000.0, InterfaceScale::Percent200);
        // by height: floor(2000 * 0.3 * 16/9) = 1066; by width: (900 - 56 - 64 - 24) / 2 = 378.
        assert_eq!(
            layout.dock,
            DockLayout::Band {
                monitor_width: 378.0
            }
        );
        let DockLayout::Band { monitor_width } = layout.dock else {
            panic!("expected a band");
        };
        assert!(
            2.0 * monitor_width + 2.0 * (BAND_GAP / 2.0 + BAND_GUTTER) * 2.0 + 56.0 <= 900.0 + EPS
        );
    }
}
