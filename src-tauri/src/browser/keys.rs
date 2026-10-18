//! Key names as the model writes them (`Enter`, `Control+a`, `Shift+Tab`) and
//! the key events a page receives for them.
//!
//! The names are the DOM's `KeyboardEvent.key` values, the spelling Playwright
//! and the DevTools Protocol use, so a model that knows either knows these.

/// Modifier bits as the DevTools Protocol counts them.
const ALT: u32 = 1;
const CONTROL: u32 = 2;
const META: u32 = 4;
const SHIFT: u32 = 8;

/// One key press, ready for `Input.dispatchKeyEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Press {
    pub key: String,
    pub code: String,
    /// The Windows virtual-key code: what Chromium on Windows decides
    /// shortcuts and editing commands by.
    pub key_code: u32,
    pub modifiers: u32,
    /// What the key types, when it types something and no modifier other
    /// than Shift is held.
    pub text: Option<String>,
}

/// Named keys: `key`, `code`, virtual-key code, and what the key types.
const NAMED: &[(&str, &str, u32, Option<&str>)] = &[
    ("Enter", "Enter", 13, Some("\r")),
    ("Tab", "Tab", 9, None),
    ("Escape", "Escape", 27, None),
    ("Backspace", "Backspace", 8, None),
    ("Delete", "Delete", 46, None),
    ("ArrowUp", "ArrowUp", 38, None),
    ("ArrowDown", "ArrowDown", 40, None),
    ("ArrowLeft", "ArrowLeft", 37, None),
    ("ArrowRight", "ArrowRight", 39, None),
    ("Home", "Home", 36, None),
    ("End", "End", 35, None),
    ("PageUp", "PageUp", 33, None),
    ("PageDown", "PageDown", 34, None),
    ("Insert", "Insert", 45, None),
    (" ", "Space", 32, Some(" ")),
];

/// Other spellings a model reaches for.
const ALIASES: &[(&str, &str)] = &[
    ("Return", "Enter"),
    ("Esc", "Escape"),
    ("Space", " "),
    ("Up", "ArrowUp"),
    ("Down", "ArrowDown"),
    ("Left", "ArrowLeft"),
    ("Right", "ArrowRight"),
    ("Del", "Delete"),
];

/// Reads `Control+Shift+K` and the like.
pub fn parse(combo: &str) -> Result<Press, String> {
    let combo = combo.trim();
    if combo.is_empty() {
        return Err("the key is empty".into());
    }
    // `+` alone, or as the last key (`Control++`), is the plus key.
    let (mods, key) = match combo.strip_suffix("++") {
        Some(rest) => (rest, "+"),
        None if combo == "+" => ("", "+"),
        None => match combo.rsplit_once('+') {
            Some((mods, key)) => (mods, key),
            None => ("", combo),
        },
    };
    let mut modifiers = 0;
    for part in mods.split('+').filter(|part| !part.is_empty()) {
        modifiers |= match part.to_ascii_lowercase().as_str() {
            "alt" | "option" => ALT,
            "control" | "ctrl" => CONTROL,
            "meta" | "cmd" | "command" | "win" => META,
            "shift" => SHIFT,
            other => return Err(format!("\"{other}\" is not a modifier: use Alt, Control, Meta or Shift")),
        };
    }

    let key = ALIASES
        .iter()
        .find(|(alias, _)| alias.eq_ignore_ascii_case(key))
        .map_or(key, |(_, name)| *name);
    let typing = modifiers & (ALT | CONTROL | META) == 0;

    if let Some(&(name, code, key_code, text)) = NAMED.iter().find(|(name, ..)| *name == key) {
        return Ok(Press {
            key: name.into(),
            code: code.into(),
            key_code,
            modifiers,
            text: text.filter(|_| typing).map(str::to_string),
        });
    }
    if let Some(n) = key.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
        if (1..=12).contains(&n) {
            return Ok(Press {
                key: key.into(),
                code: key.into(),
                key_code: 111 + n,
                modifiers,
                text: None,
            });
        }
    }

    let mut chars = key.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return Err(format!("unknown key \"{key}\""));
    };
    let (code, key_code) = if c.is_ascii_alphabetic() {
        let upper = c.to_ascii_uppercase();
        (format!("Key{upper}"), upper as u32)
    } else if c.is_ascii_digit() {
        (format!("Digit{c}"), c as u32)
    } else {
        // Punctuation: the page sees the character; the physical key is not
        // worth guessing across keyboard layouts.
        (String::new(), 0)
    };
    // Shift+a types "A", as it would on a keyboard.
    let shown = if modifiers & SHIFT != 0 { c.to_uppercase().collect() } else { c.to_string() };
    Ok(Press {
        key: shown.clone(),
        code,
        key_code,
        modifiers,
        text: typing.then_some(shown),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_and_their_aliases() {
        let enter = parse("Enter").unwrap();
        assert_eq!((enter.key.as_str(), enter.key_code, enter.text.as_deref()), ("Enter", 13, Some("\r")));
        assert_eq!(parse("return").unwrap(), enter);
        assert_eq!(parse("Esc").unwrap().key, "Escape");
        assert_eq!(parse("Space").unwrap().text.as_deref(), Some(" "));
        assert_eq!(parse("F5").unwrap().key_code, 116);
    }

    #[test]
    fn modifiers_combine_and_stop_the_key_from_typing() {
        let select_all = parse("Control+a").unwrap();
        assert_eq!(select_all.modifiers, CONTROL);
        assert_eq!((select_all.code.as_str(), select_all.key_code), ("KeyA", 65));
        assert_eq!(select_all.text, None);

        let back = parse("Shift+Tab").unwrap();
        assert_eq!(back.modifiers, SHIFT);
        assert_eq!(parse("ctrl+shift+K").unwrap().modifiers, CONTROL | SHIFT);
        assert_eq!(parse("Shift+a").unwrap().text.as_deref(), Some("A"));
    }

    #[test]
    fn plus_is_a_key_too() {
        assert_eq!(parse("+").unwrap().key, "+");
        let zoom = parse("Control++").unwrap();
        assert_eq!((zoom.key.as_str(), zoom.modifiers), ("+", CONTROL));
    }

    #[test]
    fn nonsense_is_refused() {
        assert!(parse("").is_err());
        assert!(parse("Hyper+a").is_err());
        assert!(parse("Banana").is_err());
    }
}
