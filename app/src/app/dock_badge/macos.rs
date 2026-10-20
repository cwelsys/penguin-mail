//! The Dock keeps the All Inboxes unread count even without an open window.
//! `crate::unread` counts; this only draws the number.

use std::cell::Cell;

use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;
use objc2_foundation::NSString;

pub(in crate::app) struct Badge {
    /// The number last drawn, so an unchanged count does not redraw.
    shown: Cell<Option<i64>>,
}

impl Badge {
    /// The Dock knows the app by its process; the id is for the Linux
    /// badge, which has to name a desktop file.
    pub(in crate::app) fn new(_app_id: &str) -> Badge {
        Badge {
            shown: Cell::new(None),
        }
    }

    /// Takes the number off the icon before the process ends.
    pub(in crate::app) fn hide_now(&self) {
        self.show(0);
    }

    /// Shows `unread` on the app icon, and nothing at zero.
    pub(in crate::app) fn show(&self, unread: i64) {
        let unread = unread.max(0);
        if self.shown.replace(Some(unread)) == Some(unread) {
            return;
        }
        let label = (unread > 0).then(|| NSString::from_str(&unread.to_string()));
        let mtm = MainThreadMarker::new().expect("the Dock updates on the GTK thread");
        NSApplication::sharedApplication(mtm)
            .dockTile()
            .setBadgeLabel(label.as_deref());
    }
}
