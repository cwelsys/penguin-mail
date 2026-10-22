//! Text Size beyond the message page: the thread list, the mailbox
//! sidebar and the composer's body grow by the factor the page zooms by.

use std::cell::OnceCell;

use gtk::gdk;

use crate::settings::TextSize;

/// The size style.css gives the composer's body. The rule here takes its
/// place, so it starts from the same size.
const COMPOSER_BODY: f64 = 1.04;

thread_local! {
    /// One stylesheet on the display for every window, so a composer that
    /// opens later starts at the chosen size.
    static SHEET: OnceCell<gtk::CssProvider> = const { OnceCell::new() };
}

/// Sizes the lists and the composer's body for `size`.
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

/// The lists take the factor in em, and their rows' labels, sized in em
/// themselves, follow.
fn stylesheet(size: TextSize) -> String {
    let zoom = size.zoom();
    format!(
        ".mailboxes, .thread-list {{ font-size: {zoom:.3}em; }}\n\
         textview.composer-body {{ font-size: {:.3}em; }}\n",
        COMPOSER_BODY * zoom
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keeps_the_sizes_style_css_sets() {
        assert_eq!(
            stylesheet(TextSize::Normal),
            ".mailboxes, .thread-list { font-size: 1.000em; }\n\
             textview.composer-body { font-size: 1.040em; }\n"
        );
    }

    #[test]
    fn small_shrinks_the_lists_and_the_composer_body_by_the_page_zoom() {
        assert_eq!(
            stylesheet(TextSize::Small),
            ".mailboxes, .thread-list { font-size: 0.900em; }\n\
             textview.composer-body { font-size: 0.936em; }\n"
        );
    }
}
