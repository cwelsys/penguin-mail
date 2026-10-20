//! The badge a Linux dock draws on the app icon, through the Unity
//! LauncherEntry signal that Dash to Dock, Ubuntu Dock, Dash to Panel,
//! KDE Plasma and elementary's dock listen for. `crate::unread` counts;
//! this only sends the number.

use std::cell::{Cell, OnceCell};

use gtk::glib::prelude::*;
use gtk::{gio, glib};

const INTERFACE: &str = "com.canonical.Unity.LauncherEntry";

pub(in crate::app) struct Badge {
    /// The id the application registered, which names the desktop file
    /// the dock matches to the icon. The demo registers its own, so its
    /// sample count names a desktop file no dock has.
    app_id: String,
    /// The number last sent, so an unchanged count sends nothing.
    shown: Cell<Option<i64>>,
    /// The session bus, looked up at the first count. `None` once that
    /// lookup failed: a desktop with no session bus has no dock to tell.
    bus: OnceCell<Option<gio::DBusConnection>>,
}

impl Badge {
    pub(in crate::app) fn new(app_id: &str) -> Badge {
        Badge {
            app_id: app_id.to_string(),
            shown: Cell::new(None),
            bus: OnceCell::new(),
        }
    }

    /// Sends `unread` to the dock, hidden at zero, when it differs from
    /// the count last sent.
    pub(in crate::app) fn show(&self, unread: i64) {
        if self.due(unread) {
            self.send(unread);
        }
    }

    /// Hides the badge before the process ends. A signal only queues for
    /// the bus thread, so without the flush the zero would be lost and
    /// the dock would keep the last count.
    pub(in crate::app) fn hide_now(&self) {
        if !self.due(0) {
            return;
        }
        self.send(0);
        if let Some(Some(bus)) = self.bus.get() {
            let _ = bus.flush_sync(gio::Cancellable::NONE);
        }
    }

    /// Whether `unread` differs from the count last sent, recording it.
    fn due(&self, unread: i64) -> bool {
        self.shown.replace(Some(unread.max(0))) != Some(unread.max(0))
    }

    fn send(&self, unread: i64) {
        let Some(bus) = self.bus.get_or_init(|| {
            gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)
                .inspect_err(|err| tracing::debug!(%err, "no session bus for the dock badge"))
                .ok()
        }) else {
            return;
        };
        let uri = app_uri(&self.app_id);
        if let Err(err) = bus.emit_signal(
            None,
            &object_path(&uri),
            INTERFACE,
            "Update",
            Some(&update(&self.app_id, unread)),
        ) {
            tracing::debug!(%err, "the dock badge did not go out");
        }
    }
}

/// The desktop file's URI, as the dock matches it to the icon.
fn app_uri(app_id: &str) -> String {
    format!("application://{app_id}.desktop")
}

/// The signal's arguments: the desktop file's URI and the count, shown
/// only above zero.
fn update(app_id: &str, unread: i64) -> glib::Variant {
    let unread = unread.max(0);
    let props = glib::VariantDict::new(None);
    props.insert("count", unread);
    props.insert("count-visible", unread > 0);
    // A tuple made from a Rust pair would box the dictionary as `v`,
    // which is not the `(sa{sv})` the dock reads.
    glib::Variant::tuple_from_iter([app_uri(app_id).to_variant(), props.end()])
}

/// The object path libunity used: the URI's GLib string hash under the
/// launcher entry prefix. Docks take the signal from any path, but one
/// that reads like libunity's suits a stricter listener.
fn object_path(uri: &str) -> String {
    let hash = uri.bytes().fold(5381u32, |h, c| {
        h.wrapping_shl(5).wrapping_add(h).wrapping_add(u32::from(c))
    });
    format!("/com/canonical/unity/launcherentry/{hash}")
}

#[cfg(test)]
mod tests {
    use super::*;

    type Props = std::collections::HashMap<String, glib::Variant>;

    #[test]
    fn the_update_names_the_desktop_file_and_hides_at_zero() {
        let shown = update("io.github.c9dev.PenguinMail", 7);
        assert_eq!(shown.type_().as_str(), "(sa{sv})", "what the dock expects");
        let (uri, props) = shown.get::<(String, Props)>().unwrap();
        assert_eq!(uri, "application://io.github.c9dev.PenguinMail.desktop");
        assert_eq!(props["count"].get::<i64>(), Some(7));
        assert_eq!(props["count-visible"].get::<bool>(), Some(true));

        let hidden = update("io.github.c9dev.penguin-mail", 0);
        let (uri, props) = hidden.get::<(String, Props)>().unwrap();
        assert_eq!(uri, "application://io.github.c9dev.penguin-mail.desktop");
        assert_eq!(props["count-visible"].get::<bool>(), Some(false));
    }

    #[test]
    fn a_count_goes_out_once_until_it_changes() {
        let badge = Badge::new("io.github.c9dev.PenguinMail");
        assert!(badge.due(0), "the first count clears a badge left behind");
        assert!(!badge.due(0));
        assert!(badge.due(3));
        assert!(!badge.due(3));
        assert!(badge.due(0));
    }

    #[test]
    fn the_object_path_is_one_libunity_would_use() {
        assert_eq!(
            object_path("application://io.github.c9dev.PenguinMail.desktop"),
            "/com/canonical/unity/launcherentry/302760210"
        );
    }
}
