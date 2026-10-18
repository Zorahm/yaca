//! `act`: the gestures a person makes on a page.
//!
//! Clicks and keys are real input events, the ones a page cannot tell from a
//! person's, so a site behaves the way it would for the human. Where an
//! element is, the helpers in the page say (`page.js`); where the model only
//! has a point from a screenshot, the point is used as it is.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use super::engine::{Button, Engine, Load, LOAD_LIMIT};
use super::keys;
use super::layout::Rect;
use super::tools::{open_page, status, target, Reply};
use super::Browser;

const WAIT_DEFAULT_MS: u64 = 5_000;
const WAIT_MAX_MS: u64 = 30_000;
const SCROLL_DEFAULT: f64 = 600.0;
/// The ring drawn around a bare point, in CSS pixels.
const POINT_RING: f64 = 24.0;

/// Where a gesture lands: a point in the viewport, the box to ring, and the
/// element's own description when the page had one.
struct Aim {
    x: f64,
    y: f64,
    ring: Rect,
    label: String,
    warning: Option<String>,
}

pub async fn act(browser: &Arc<Browser>, args: &Value) -> Result<Reply, String> {
    let engine = open_page(browser)?;
    let action = args["action"].as_str().unwrap_or_default();
    let since = engine.loads();
    let done = match action {
        "click" | "double_click" | "right_click" => {
            let aim = aim(&*engine, args).await?.ok_or("say where to click: ref, selector, or x and y")?;
            let (button, count) = match action {
                "double_click" => (Button::Left, 2),
                "right_click" => (Button::Right, 1),
                _ => (Button::Left, 1),
            };
            ring(&*engine, aim.ring).await;
            engine.click(aim.x, aim.y, button, count).await?;
            with_warning(format!("{} {}.", verb(action), aim.label), aim.warning)
        }
        "hover" => {
            let aim = aim(&*engine, args).await?.ok_or("say where to hover: ref, selector, or x and y")?;
            ring(&*engine, aim.ring).await;
            engine.hover(aim.x, aim.y).await?;
            with_warning(format!("Hovered over {}.", aim.label), aim.warning)
        }
        "type" => {
            let text = args["text"].as_str().ok_or("type needs text")?;
            let label = match target(args) {
                Some(target) => {
                    let clear = args["clear"].as_bool() != Some(false);
                    let focused = engine.helper(&format!("focus({target}, {clear})")).await?;
                    let label = focused["label"].as_str().unwrap_or("the element").to_string();
                    if focused["editable"].as_bool() != Some(true) {
                        return Err(format!("{label} is not a text field"));
                    }
                    ring(&*engine, rect(&focused["rect"])).await;
                    label
                }
                None => "the focused element".to_string(),
            };
            engine.insert_text(text).await?;
            if args["submit"].as_bool() == Some(true) {
                engine.press(&keys::parse("Enter")?).await?;
            }
            format!("Typed {} characters into {label}.", text.chars().count())
        }
        "press" => {
            let key = args["text"].as_str().ok_or("press needs the key in text, such as Enter")?;
            let press = keys::parse(key)?;
            if let Some(target) = target(args) {
                let focused = engine.helper(&format!("focus({target}, false)")).await?;
                ring(&*engine, rect(&focused["rect"])).await;
            }
            engine.press(&press).await?;
            format!("Pressed {key}.")
        }
        "scroll" => {
            let dx = args["dx"].as_f64().unwrap_or(0.0);
            let dy = args["dy"].as_f64().unwrap_or(if args["dx"].is_number() { 0.0 } else { SCROLL_DEFAULT });
            match aim(&*engine, args).await? {
                // Bringing an element into view is the scroll asked for,
                // unless a distance says to scroll inside it.
                Some(aim) if dx == 0.0 && args["dy"].is_null() => {
                    ring(&*engine, aim.ring).await;
                    format!("Scrolled {} into view.", aim.label)
                }
                Some(aim) => {
                    engine.wheel(aim.x, aim.y, dx, dy).await?;
                    format!("Scrolled {} by {dx}, {dy}.", aim.label)
                }
                None => {
                    let plan = browser.plan();
                    let (x, y) = (plan.viewport.width / 2.0, plan.viewport.height / 2.0);
                    engine.wheel(x, y, dx, dy).await?;
                    format!("Scrolled the page by {dx}, {dy}.")
                }
            }
        }
        "select" => {
            let target = target(args).ok_or("select needs the <select> by ref or selector")?;
            let values: Vec<String> = match &args["values"] {
                Value::Array(list) => list.iter().filter_map(Value::as_str).map(str::to_string).collect(),
                Value::String(one) => vec![one.clone()],
                _ => Vec::new(),
            };
            if values.is_empty() {
                return Err("select needs values".into());
            }
            let chosen = engine.helper(&format!("choose({target}, {})", json!(values))).await?;
            ring(&*engine, rect(&chosen["rect"])).await;
            let picked: Vec<&str> = chosen["picked"]
                .as_array()
                .map(|list| list.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            format!(
                "Selected {} in {}.",
                picked.join(", "),
                chosen["label"].as_str().unwrap_or("the list")
            )
        }
        "wait" => wait(&*engine, args).await?,
        "" => return Err("act needs an action".into()),
        other => return Err(format!("unknown action {other}")),
    };
    let after = match engine.settle(since, LOAD_LIMIT).await? {
        Load::Nothing => "",
        Load::Done => "\nThe page navigated.",
        Load::Pending => "\nThe page navigated and is still loading.",
    };
    Ok(Reply {
        content: vec![json!({ "type": "text", "text": format!("{}{done}{after}", status(browser)) })],
        error: false,
    })
}

fn verb(action: &str) -> &'static str {
    match action {
        "double_click" => "Double-clicked",
        "right_click" => "Right-clicked",
        _ => "Clicked",
    }
}

fn with_warning(text: String, warning: Option<String>) -> String {
    match warning {
        Some(warning) => format!("{text}\nwarning: {warning}"),
        None => text,
    }
}

fn rect(value: &Value) -> Rect {
    let number = |key: &str| value[key].as_f64().unwrap_or(0.0);
    Rect {
        x: number("x"),
        y: number("y"),
        width: number("width"),
        height: number("height"),
    }
}

/// The ring is for the human: a page it cannot be drawn on is still acted on.
async fn ring(engine: &dyn Engine, rect: Rect) {
    let _ = engine.highlight(rect).await;
}

/// Where the arguments point. `None` when they point nowhere.
async fn aim(engine: &dyn Engine, args: &Value) -> Result<Option<Aim>, String> {
    if let Some(target) = target(args) {
        let found = engine.helper(&format!("point({target})")).await?;
        let label = found["label"].as_str().unwrap_or("the element").to_string();
        let mut warning = None;
        if found["disabled"].as_bool() == Some(true) {
            warning = Some(format!("{label} is disabled"));
        }
        if let Some(cover) = found["covered"].as_str() {
            warning = Some(format!("{label} is covered by {cover}, which got the event instead"));
        }
        return Ok(Some(Aim {
            x: found["x"].as_f64().unwrap_or(0.0),
            y: found["y"].as_f64().unwrap_or(0.0),
            ring: rect(&found["rect"]),
            label,
            warning,
        }));
    }
    match (args["x"].as_f64(), args["y"].as_f64()) {
        (Some(x), Some(y)) => Ok(Some(Aim {
            x,
            y,
            ring: Rect {
                x: x - POINT_RING / 2.0,
                y: y - POINT_RING / 2.0,
                width: POINT_RING,
                height: POINT_RING,
            },
            label: format!(
                "the point ({x}, {y}) [{}]",
                args["element"].as_str().unwrap_or("no element named")
            ),
            warning: None,
        })),
        (None, None) => Ok(None),
        _ => Err("give both x and y".into()),
    }
}

async fn wait(engine: &dyn Engine, args: &Value) -> Result<String, String> {
    let ms = args["timeout_ms"].as_u64().unwrap_or(WAIT_DEFAULT_MS).clamp(100, WAIT_MAX_MS);
    let query = match (args["selector"].as_str(), args["text"].as_str()) {
        (Some(selector), _) => json!({ "selector": selector }),
        (None, Some(text)) => json!({ "text": text }),
        (None, None) => {
            // Nothing named: just a pause, for an animation or a debounce.
            tokio::time::sleep(Duration::from_millis(ms)).await;
            return Ok(format!("Waited {ms} ms."));
        }
    };
    let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
    loop {
        // A page between documents has no helpers to ask; that is not yet.
        if engine.helper(&format!("present({query})")).await.ok() == Some(Value::Bool(true)) {
            return Ok("It is there.".to_string());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!("still not there after {ms} ms"));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
