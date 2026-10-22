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

/// How many pixels a width laid out for Normal text takes: the chosen
/// size times the desktop's own text scale, which GTK carries as a dpi.
/// Pane widths are kept at Normal and multiplied by this on the way to
/// the screen, so they keep their share of the text at every size.
pub fn factor(size: TextSize) -> f64 {
    let dpi = gtk::Settings::default().map_or(-1, |settings| settings.gtk_xft_dpi());
    scale(size.zoom(), dpi)
}

fn scale(zoom: f64, xft_dpi: i32) -> f64 {
    let desktop = if xft_dpi > 0 {
        f64::from(xft_dpi) / (96.0 * 1024.0)
    } else {
        1.0
    };
    zoom * desktop
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_follow_the_text_size_and_the_desktop_scale() {
        assert_eq!(scale(1.0, -1), 1.0);
        assert_eq!(scale(1.3, 96 * 1024), 1.3);
        assert_eq!(scale(1.0, 120 * 1024), 1.25);
        assert_eq!(scale(0.9, 0), 0.9);
    }

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
