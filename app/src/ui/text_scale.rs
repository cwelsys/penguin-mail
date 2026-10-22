//! Text Size beyond the message page: every window's text grows by the
//! factor the page zooms by, as the desktop's large text setting would.

use std::cell::OnceCell;

use gtk::gdk;

use crate::settings::TextSize;

thread_local! {
    /// One stylesheet on the display for every window, so a composer that
    /// opens later starts at the chosen size.
    static SHEET: OnceCell<gtk::CssProvider> = const { OnceCell::new() };
}

/// Sizes every window's text for `size`.
pub fn apply(size: TextSize) {
    SHEET.with(|sheet| {
        let provider = sheet.get_or_init(|| {
            let provider = gtk::CssProvider::new();
            if let Some(display) = gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
            provider
        });
        provider.load_from_string(&stylesheet(size));
    });
}

/// The windows take the factor in em, and every size style.css gives in
/// em under them follows.
fn stylesheet(size: TextSize) -> String {
    format!("window {{ font-size: {:.3}em; }}\n", size.zoom())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keeps_the_desktop_font_size() {
        assert_eq!(
            stylesheet(TextSize::Normal),
            "window { font-size: 1.000em; }\n"
        );
    }

    #[test]
    fn larger_grows_every_window_by_the_page_zoom() {
        assert_eq!(
            stylesheet(TextSize::Larger),
            "window { font-size: 1.300em; }\n"
        );
    }
}
