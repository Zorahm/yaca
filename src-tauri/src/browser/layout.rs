//! Where a browser's page sits and what size it is laid out at.
//!
//! The page has an area of its own, like the window of an ordinary browser,
//! and three things decide it:
//!
//! - the browser column is open on this chat: the area is the column, and the
//!   model's screenshot is what the human sees;
//! - the column is closed (or shows another chat): the page keeps living out
//!   of sight at [`DEFAULT`], so a screenshot still has a size to be taken at;
//! - the model asked for a size (`viewport`): the page is laid out at it
//!   wherever it is, and an open column shows it scaled down to fit.
//!
//! "Out of sight" is past the window's left edge, not hidden: a hidden
//! WebView2 stops drawing, and a page that does not draw cannot be
//! photographed.

use serde::{Deserialize, Serialize};

/// The size a page is laid out at when nothing else decides it.
pub const DEFAULT: Size = Size {
    width: 1280.0,
    height: 800.0,
};

/// Far enough past the window's left edge that no part of a page shows.
const AWAY: f64 = -20_000.0;

/// The model's sizes for the common devices.
pub const PRESETS: [(&str, Size, bool); 3] = [
    ("desktop", DEFAULT, false),
    ("tablet", Size { width: 820.0, height: 1180.0 }, true),
    ("mobile", Size { width: 390.0, height: 844.0 }, true),
];

/// Smallest and largest size the model may ask for, in CSS pixels.
pub const MIN_SIDE: f64 = 200.0;
pub const MAX_SIDE: f64 = 4_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// A rectangle in the window, in logical pixels: what the interface measured
/// with `getBoundingClientRect`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A size the model set with `viewport`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Forced {
    pub size: Size,
    pub mobile: bool,
}

/// What the page's area is following right now. The header of the column
/// says which, next to the size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ViewportSource {
    Panel,
    Default,
    Model,
}

/// What the engine is told to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    /// Where the webview goes in the window.
    pub bounds: Rect,
    /// Layout size that differs from `bounds`, and the scale it is drawn at.
    pub emulate: Option<Emulation>,
    /// The size the page is laid out at: what a screenshot will measure.
    pub viewport: Size,
    pub source: ViewportSource,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Emulation {
    pub size: Size,
    pub scale: f64,
    pub mobile: bool,
}

/// The plan for a page shown at `shown` (`None`: not shown), with `forced`
/// the size the model asked for.
pub fn plan(shown: Option<Rect>, forced: Option<Forced>) -> Plan {
    match (shown, forced) {
        (Some(rect), None) => Plan {
            bounds: rect,
            emulate: None,
            viewport: Size {
                width: rect.width.round(),
                height: rect.height.round(),
            },
            source: ViewportSource::Panel,
        },
        (None, None) => Plan {
            bounds: away(DEFAULT),
            emulate: None,
            viewport: DEFAULT,
            source: ViewportSource::Default,
        },
        (Some(rect), Some(forced)) => {
            // Scaled down to fit, never up: a page larger than its column is
            // the case the scale is for, and a small one is shown as it is.
            let scale = (rect.width / forced.size.width)
                .min(rect.height / forced.size.height)
                .min(1.0);
            Plan {
                bounds: rect,
                emulate: Some(Emulation {
                    size: forced.size,
                    scale,
                    mobile: forced.mobile,
                }),
                viewport: forced.size,
                source: ViewportSource::Model,
            }
        }
        (None, Some(forced)) => Plan {
            bounds: away(forced.size),
            // The webview has the size already; emulation is only for
            // `mobile`, which no size of a window gives.
            emulate: forced.mobile.then_some(Emulation {
                size: forced.size,
                scale: 1.0,
                mobile: true,
            }),
            viewport: forced.size,
            source: ViewportSource::Model,
        },
    }
}

fn away(size: Size) -> Rect {
    Rect {
        x: AWAY,
        y: 0.0,
        width: size.width,
        height: size.height,
    }
}

/// The size the model asked for, from `viewport`'s arguments: a preset name,
/// or both sides.
pub fn forced(preset: Option<&str>, width: Option<f64>, height: Option<f64>) -> Result<Option<Forced>, String> {
    match (preset, width, height) {
        (Some("panel"), None, None) => Ok(None),
        (Some(name), None, None) => PRESETS
            .iter()
            .find(|(preset, ..)| *preset == name)
            .map(|&(_, size, mobile)| Some(Forced { size, mobile }))
            .ok_or_else(|| format!("unknown preset \"{name}\": use desktop, tablet, mobile or panel")),
        (None, Some(width), Some(height)) => {
            let side = |value: f64| (MIN_SIDE..=MAX_SIDE).contains(&value);
            if !side(width) || !side(height) {
                return Err(format!(
                    "width and height must be between {MIN_SIDE} and {MAX_SIDE} CSS pixels"
                ));
            }
            Ok(Some(Forced {
                size: Size {
                    width: width.round(),
                    height: height.round(),
                },
                mobile: false,
            }))
        }
        _ => Err("give either a preset, or both width and height".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLUMN: Rect = Rect {
        x: 600.0,
        y: 48.0,
        width: 500.4,
        height: 700.0,
    };

    #[test]
    fn an_open_column_is_the_page() {
        let plan = plan(Some(COLUMN), None);
        assert_eq!(plan.bounds, COLUMN);
        assert_eq!(plan.emulate, None);
        assert_eq!(plan.viewport, Size { width: 500.0, height: 700.0 });
        assert_eq!(plan.source, ViewportSource::Panel);
    }

    #[test]
    fn a_closed_column_keeps_the_page_out_of_sight_at_the_default_size() {
        let plan = plan(None, None);
        assert!(plan.bounds.x + plan.bounds.width < 0.0, "{:?}", plan.bounds);
        assert_eq!((plan.bounds.width, plan.bounds.height), (1280.0, 800.0));
        assert_eq!(plan.viewport, DEFAULT);
        assert_eq!(plan.source, ViewportSource::Default);
    }

    #[test]
    fn the_models_size_is_drawn_scaled_down_into_the_column() {
        let big = Forced {
            size: Size { width: 1000.0, height: 700.0 },
            mobile: false,
        };
        let plan = plan(Some(COLUMN), Some(big));
        let emulate = plan.emulate.unwrap();
        assert_eq!(emulate.size, big.size);
        assert!((emulate.scale - 0.5004).abs() < 1e-9, "{}", emulate.scale);
        assert_eq!(plan.viewport, big.size);
        assert_eq!(plan.source, ViewportSource::Model);

        let small = Forced {
            size: Size { width: 390.0, height: 600.0 },
            mobile: true,
        };
        assert_eq!(super::plan(Some(COLUMN), Some(small)).emulate.unwrap().scale, 1.0);
    }

    #[test]
    fn out_of_sight_the_models_size_is_the_webviews() {
        let mobile = forced(Some("mobile"), None, None).unwrap().unwrap();
        let plan = plan(None, Some(mobile));
        assert_eq!((plan.bounds.width, plan.bounds.height), (390.0, 844.0));
        assert!(plan.emulate.unwrap().mobile);
        let desktop = forced(None, Some(1440.0), Some(900.0)).unwrap().unwrap();
        assert_eq!(super::plan(None, Some(desktop)).emulate, None);
    }

    #[test]
    fn the_size_asked_for_is_checked() {
        assert_eq!(forced(Some("panel"), None, None).unwrap(), None);
        assert!(forced(Some("watch"), None, None).is_err());
        assert!(forced(None, Some(100.0), Some(800.0)).is_err());
        assert!(forced(None, Some(800.0), None).is_err());
        assert!(forced(Some("mobile"), Some(800.0), Some(600.0)).is_err());
    }
}
