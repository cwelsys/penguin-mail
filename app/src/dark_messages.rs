//! Dark message mode: takes out of a message the colors that only read on
//! a white page, so it can sit on the window's dark colors. Brand colors
//! with enough contrast stay. The rules are Thunderbird's dark message
//! mode (`DarkReader.mjs`), applied to declarations as they are written
//! rather than to computed styles.

use std::sync::atomic::{AtomicBool, Ordering};

static WANTED: AtomicBool = AtomicBool::new(false);
static WINDOW_DARK: AtomicBool = AtomicBool::new(false);

/// Follows the preference.
pub fn set_wanted(on: bool) {
    WANTED.store(on, Ordering::Relaxed);
}

/// Follows the window's light or dark style.
pub fn set_window_dark(dark: bool) {
    WINDOW_DARK.store(dark, Ordering::Relaxed);
}

/// Whether bodies are adapted now. Cleaning runs on a worker thread, which
/// cannot ask GTK or the settings.
pub fn on() -> bool {
    WANTED.load(Ordering::Relaxed) && WINDOW_DARK.load(Ordering::Relaxed)
}

type Rgb = [f64; 3];

const WHITE: Rgb = [255.0; 3];
/// A color brighter than this is light.
const LIGHT: f64 = 200.0;
/// The least contrast a background that stays has with its text.
const CONTRAST: f64 = 3.5;

/// The declarations of one `style` attribute or rule block without the
/// ones written for a white page: blend modes, background pictures and
/// gradients, a background that is light or too close to its text, and
/// dark text left without a background of its own.
pub fn adapt(style: &str) -> String {
    let all = declarations(style);
    let mut gone = vec![false; all.len()];
    let mut backgrounds = Vec::new();
    let mut colors = Vec::new();
    for (at, declaration) in all.iter().enumerate() {
        let Some((name, value)) = declaration.split_once(':') else {
            continue;
        };
        let value = value.to_ascii_lowercase();
        match name.trim().to_ascii_lowercase().as_str() {
            "mix-blend-mode" | "background-image" => gone[at] = true,
            "background" | "background-color" => {
                if value.contains("gradient(") || value.contains("url(") {
                    gone[at] = true;
                } else if let Some(rgb) = color(&value) {
                    backgrounds.push((at, rgb));
                }
            }
            "color" => {
                if let Some(rgb) = color(&value) {
                    colors.push((at, rgb));
                }
            }
            _ => {}
        }
    }
    // Text with no color of its own takes the page's, which is white.
    let text = colors.last().map_or(WHITE, |(_, rgb)| *rgb);
    let stays = backgrounds
        .last()
        .is_some_and(|(_, rgb)| luminance(*rgb) <= LIGHT && contrast(*rgb, text) >= CONTRAST);
    if !stays {
        for (at, _) in &backgrounds {
            gone[*at] = true;
        }
        for (at, rgb) in &colors {
            gone[*at] |= luminance(*rgb) <= LIGHT;
        }
    }
    if !gone.contains(&true) {
        return style.to_string();
    }
    let kept: Vec<&str> = all
        .iter()
        .zip(gone)
        .filter_map(|(declaration, gone)| (!gone).then_some(*declaration))
        .collect();
    kept.join(";")
}

/// `style` split at the semicolons that end a declaration, leaving alone
/// the ones inside a string or a `url(...)`.
fn declarations(style: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut quote = None;
    let mut start = 0;
    for (at, c) in style.char_indices() {
        match (quote, c) {
            (Some(open), c) if c == open => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => depth = depth.saturating_sub(1),
            (None, ';') if depth == 0 => {
                out.push(&style[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    out.push(&style[start..]);
    out.retain(|declaration| !declaration.trim().is_empty());
    out
}

/// The color a lower-case value names, when it is one this module reads.
fn color(value: &str) -> Option<Rgb> {
    if let Some(at) = value.find("rgb") {
        let inside = value[at..].split_once('(')?.1.split(')').next()?;
        let mut parts = inside
            .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
            .filter(|part| !part.is_empty());
        let mut channel = || {
            let part = parts.next()?;
            match part.strip_suffix('%') {
                Some(share) => share.parse::<f64>().ok().map(|share| share * 2.55),
                None => part.parse().ok(),
            }
        };
        return Some([channel()?, channel()?, channel()?]);
    }
    value
        .split(|c: char| c.is_whitespace() || c == '!')
        .find_map(word)
}

fn word(word: &str) -> Option<Rgb> {
    let Some(hex) = word.strip_prefix('#') else {
        return NAMED
            .iter()
            .find(|(name, _)| *name == word)
            .map(|(_, rgb)| rgb.map(f64::from));
    };
    let digits = |at: usize, len: usize| {
        let value = u8::from_str_radix(hex.get(at..at + len)?, 16).ok()?;
        Some(f64::from(value) * if len == 1 { 17.0 } else { 1.0 })
    };
    match hex.len() {
        3 | 4 => Some([digits(0, 1)?, digits(1, 1)?, digits(2, 1)?]),
        6 | 8 => Some([digits(0, 2)?, digits(2, 2)?, digits(4, 2)?]),
        _ => None,
    }
}

// shortcut: the 16 HTML names, orange and the light greys; a color under
// any other CSS name stays as written, so add names as mail turns up with them.
const NAMED: [(&str, [u8; 3]); 22] = [
    ("black", [0, 0, 0]),
    ("silver", [192, 192, 192]),
    ("gray", [128, 128, 128]),
    ("grey", [128, 128, 128]),
    ("white", [255, 255, 255]),
    ("maroon", [128, 0, 0]),
    ("red", [255, 0, 0]),
    ("purple", [128, 0, 128]),
    ("fuchsia", [255, 0, 255]),
    ("green", [0, 128, 0]),
    ("lime", [0, 255, 0]),
    ("olive", [128, 128, 0]),
    ("yellow", [255, 255, 0]),
    ("navy", [0, 0, 128]),
    ("blue", [0, 0, 255]),
    ("teal", [0, 128, 128]),
    ("aqua", [0, 255, 255]),
    ("orange", [255, 165, 0]),
    ("whitesmoke", [245, 245, 245]),
    ("lightgray", [211, 211, 211]),
    ("lightgrey", [211, 211, 211]),
    ("darkgray", [169, 169, 169]),
];

fn luminance([r, g, b]: Rgb) -> f64 {
    0.2125 * r + 0.7154 * g + 0.0721 * b
}

/// The WCAG contrast ratio of two colors.
fn contrast(a: Rgb, b: Rgb) -> f64 {
    let relative = |rgb: Rgb| {
        let [r, g, b] = rgb.map(|c| {
            let c = c / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * r + 0.7152 * g + 0.0722 * b
    };
    let (a, b) = (relative(a), relative(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::adapt;

    #[test]
    fn a_light_background_and_its_dark_text_go() {
        assert_eq!(adapt("background-color:#ffffff;color:#222;padding:4px"), "padding:4px");
        assert_eq!(adapt("background: White ; color: rgb(34, 34, 34)"), "");
        assert_eq!(adapt("color:#333 !important;margin:0"), "margin:0");
    }

    #[test]
    fn light_text_stays() {
        assert_eq!(adapt("color:#eee"), "color:#eee");
        assert_eq!(adapt("color:rgba(100%,100%,100%,0.8)"), "color:rgba(100%,100%,100%,0.8)");
    }

    #[test]
    fn a_brand_background_stays_with_text_that_reads_on_it() {
        let button = "background:#1a73e8;color:#fff;border-radius:4px";
        assert_eq!(adapt(button), button);
        let dark_on_orange = "background-color:#ff9900;color:#111";
        assert_eq!(adapt(dark_on_orange), dark_on_orange);
        assert_eq!(adapt("background:#1a73e8"), "background:#1a73e8");
    }

    #[test]
    fn a_background_too_close_to_its_text_goes() {
        assert_eq!(adapt("background:#777;color:#555"), "");
        assert_eq!(adapt("background:#ff9900"), "", "white text would not read on it");
    }

    #[test]
    fn pictures_gradients_and_blend_modes_go() {
        assert_eq!(
            adapt("background:url('a;b.png') #fff;mix-blend-mode:multiply;width:10px"),
            "width:10px"
        );
        assert_eq!(adapt("background-image:linear-gradient(#fff,#eee);color:#fff"), "color:#fff");
        assert_eq!(adapt("background:linear-gradient(red,blue)"), "");
    }

    #[test]
    fn the_last_of_two_backgrounds_decides() {
        assert_eq!(adapt("background:#fff;background:#202124;color:#e8eaed"),
            "background:#fff;background:#202124;color:#e8eaed");
        assert_eq!(adapt("background:#202124;background:#fff"), "");
    }

    #[test]
    fn what_it_cannot_read_stays() {
        for style in [
            "color:var(--ink)",
            "background:transparent",
            "border-color:#000;font:12px Arial",
            "color:darkslategray",
        ] {
            assert_eq!(adapt(style), style);
        }
    }
}
