//! The desktop's palette in place of the mockup's, in `data/palette.css`.

use gtk::gdk;

pub fn load(display: &gdk::Display) {
    let css = gtk::CssProvider::new();
    css.load_from_string(include_str!("../data/palette.css"));
    gtk::style_context_add_provider_for_display(
        display,
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2,
    );
}
