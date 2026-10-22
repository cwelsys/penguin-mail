//! The desktop's palette in place of the mockup's: `data/palette.css` for
//! the widgets, and the view and accent colours GTK resolves for the
//! conversation page, which WebKit draws on its own.

use gtk::gdk;
use gtk::prelude::*;
use std::sync::Mutex;

/// The page's colour variables as last resolved, or empty before the
/// first page. Pages are built from a `Theme` that cannot carry them.
static PAGE: Mutex<String> = Mutex::new(String::new());
static VIEW_BG: Mutex<Option<gdk::RGBA>> = Mutex::new(None);

pub fn load(display: &gdk::Display) {
    let css = gtk::CssProvider::new();
    css.load_from_string(include_str!("../data/palette.css"));
    gtk::style_context_add_provider_for_display(
        display,
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2,
    );
}

/// A colour the GTK theme or the user's gtk.css names, in the current
/// light or dark style. GTK has no other way to read one by name.
fn named(name: &str) -> Option<gdk::RGBA> {
    #[allow(deprecated)]
    gtk::Label::new(None).style_context().lookup_color(name)
}

/// The accent for shapes drawn in code. libadwaita's own reads the
/// desktop's accent setting, which a palette in gtk.css does not change.
pub fn accent() -> gdk::RGBA {
    named("accent_bg_color").unwrap_or_else(|| adw::StyleManager::default().accent_color_rgba())
}

/// The accent as text, for links in the composer.
pub fn link() -> String {
    named("accent_color").map_or_else(|| "#1c71d8".to_string(), |c| c.to_str().to_string())
}

/// Quoted text in the composer: the view's text at 60 %, as the page's
/// dim text is.
pub fn quote() -> String {
    named("view_fg_color").map_or_else(
        || "#777777".to_string(),
        |c| c.with_alpha(0.6).to_str().to_string(),
    )
}

/// Reads the view and accent colours of the current light or dark style.
/// Runs on the GTK thread before a page is built.
pub fn follow() {
    let (Some(bg), Some(fg)) = (named("view_bg_color"), named("view_fg_color")) else {
        return;
    };
    let accent = named("accent_bg_color").map(|c| c.to_str().to_string());
    let accent_text = named("accent_color").map(|c| c.to_str().to_string());
    let mut css = page_vars(
        &bg.to_str(),
        &fg.to_str(),
        accent.as_deref(),
        accent_text.as_deref(),
    );
    if adw::StyleManager::default().is_dark() {
        css.push_str(&dark_avatars());
    }
    *PAGE.lock().unwrap() = css;
    *VIEW_BG.lock().unwrap() = Some(bg);
}

/// `format::PALETTE` against Catppuccin Mocha, as `data/palette.css` maps
/// the window's avatars.
const MOCHA: [(&str, &str); 9] = [
    ("#3584e4", "#89b4fa"),
    ("#2190a4", "#94e2d5"),
    ("#3a944a", "#a6e3a1"),
    ("#c88800", "#f9e2af"),
    ("#ed5b00", "#fab387"),
    ("#e62d42", "#f38ba8"),
    ("#d56199", "#f5c2e7"),
    ("#9141ac", "#cba6f7"),
    ("#6f8396", "#9399b2"),
];

/// The page writes an avatar's hue into its `style` attribute, which only
/// `!important` outranks.
fn dark_avatars() -> String {
    MOCHA
        .iter()
        .map(|(from, to)| {
            format!(".avatar[style*=\"{from}\"]{{background:{to}!important;color:#11111b}}")
        })
        .collect()
}

/// CSS appended to the page's own, overriding its colour variables.
pub fn page() -> String {
    PAGE.lock().unwrap().clone()
}

/// The WebView's ground, shown before a page paints.
pub fn view_bg() -> Option<gdk::RGBA> {
    *VIEW_BG.lock().unwrap()
}

/// The page palette (`render::PagePalette`) built from two colours, with
/// the same steps between them in light and dark.
fn page_vars(bg: &str, fg: &str, accent: Option<&str>, accent_text: Option<&str>) -> String {
    let mix = |share: u8| format!("color-mix(in srgb,{fg} {share}%,transparent)");
    let mut css = format!(
        ":root{{--bg:{bg};--fg:{fg};--dim:{dim};--card:{card};--line:{line};--hover:{hover};\
         --surface:color-mix(in srgb,{fg} 6%,{bg})",
        dim = mix(60),
        card = mix(8),
        line = mix(9),
        hover = mix(4),
    );
    if let Some(accent) = accent {
        css.push_str(&format!(";--accent:{accent}"));
    }
    if let Some(accent_text) = accent_text {
        css.push_str(&format!(";--accent-text:{accent_text}"));
    }
    css.push('}');
    css
}

#[cfg(test)]
mod tests {
    use super::page_vars;

    #[test]
    fn the_page_takes_the_view_colours_and_derives_the_rest_from_them() {
        let css = page_vars(
            "rgb(24,24,37)",
            "rgb(205,214,244)",
            Some("rgb(137,180,250)"),
            None,
        );
        assert!(css.starts_with(":root{--bg:rgb(24,24,37);--fg:rgb(205,214,244);"));
        assert!(css.contains("--dim:color-mix(in srgb,rgb(205,214,244) 60%,transparent)"));
        assert!(css.contains("--surface:color-mix(in srgb,rgb(205,214,244) 6%,rgb(24,24,37))"));
        assert!(css.contains("--accent:rgb(137,180,250)"));
        assert!(!css.contains("--accent-text"));
        assert!(css.ends_with('}'));
    }

    #[test]
    fn every_avatar_hue_the_page_writes_has_a_mocha_twin() {
        let froms: Vec<&str> = super::MOCHA.iter().map(|(from, _)| *from).collect();
        assert_eq!(froms, crate::format::PALETTE);
        assert!(
            super::dark_avatars().contains(
                ".avatar[style*=\"#e62d42\"]{background:#f38ba8!important;color:#11111b}"
            )
        );
    }
}
