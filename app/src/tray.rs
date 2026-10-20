//! The StatusNotifierItem shown by Ubuntu's AppIndicator extension.

use crate::APP_ID;
use mailrs_domain::AccountId;
use mailrs_domain::translate::{fill, fill_plural, gettext};

pub enum TrayCommand {
    Toggle,
    Open,
    /// Show the window on this account's inbox.
    OpenInbox(AccountId),
    Compose,
    Check,
    CheckForUpdates,
    InstallUpdate,
    WhatsNew,
    Quit,
}

/// What the tray says it is holding. A screen reader reads the title, so
/// the count is a sentence rather than a number beside a word.
fn summary(unread: i64) -> String {
    match unread {
        ..=0 => gettext("No unread mail"),
        n => fill_plural(
            "{count} unread message",
            "{count} unread messages",
            n as usize,
            &[("count", &n.to_string())],
        ),
    }
}

/// One account's line in the tray menu. The count sat two spaces after
/// the address, which reads out as a bare number.
fn account_line(email: &str, unread: i64) -> String {
    match unread {
        ..=0 => email.to_string(),
        n => fill_plural(
            "{account}, {count} unread message",
            "{account}, {count} unread messages",
            n as usize,
            &[("account", email), ("count", &n.to_string())],
        ),
    }
}

/// One account's unread INBOX count, with what the tray needs to name it
/// and to open it.
#[derive(Clone, Debug, PartialEq)]
pub struct AccountUnread {
    pub id: AccountId,
    pub email: String,
    pub unread: i64,
}

/// A line above the menu's separator.
#[derive(Debug, PartialEq)]
enum AccountLine {
    /// An account with unread mail; choosing it opens that inbox.
    Unread { id: AccountId, label: String },
    /// Nothing is unread anywhere.
    NoUnread(String),
}

/// The lines above the separator: the accounts with unread mail in the
/// order given, or one line saying there is none. No accounts, no lines.
fn account_lines(accounts: &[AccountUnread]) -> Vec<AccountLine> {
    if accounts.is_empty() {
        return Vec::new();
    }
    let lines: Vec<AccountLine> = accounts
        .iter()
        .filter(|a| a.unread > 0)
        .map(|a| AccountLine::Unread {
            id: a.id,
            label: account_line(&a.email, a.unread),
        })
        .collect();
    if lines.is_empty() {
        vec![AccountLine::NoUnread(summary(0))]
    } else {
        lines
    }
}

/// The app icon at the sizes trays draw, for a host that cannot find the
/// icon by name. A snap's or Flatpak's icons are not on the host, and
/// Cinnamon's tray shows a blank picture when neither the name nor a
/// pixmap gives it one. The colour icon reads on light and dark panels,
/// where a host that does not recolor symbolic icons leaves the glyph dark.
const PIXMAPS: [(u32, &[u8]); 5] = [
    (16, include_bytes!("../data/tray/16.png")),
    (22, include_bytes!("../data/tray/22.png")),
    (24, include_bytes!("../data/tray/24.png")),
    (32, include_bytes!("../data/tray/32.png")),
    (48, include_bytes!("../data/tray/48.png")),
];

/// `rgba`, rows of `stride` bytes holding `width` pixels of straight
/// RGBA, as the ARGB32 in network byte order that a tray pixmap takes.
/// Empty when `rgba` holds fewer rows than `height`.
fn argb_from_rgba(width: usize, height: usize, stride: usize, rgba: &[u8]) -> Vec<u8> {
    let row = width * 4;
    if height > 0 && (stride < row || rgba.len() < stride * (height - 1) + row) {
        return Vec::new();
    }
    let mut argb = Vec::with_capacity(row * height);
    for y in 0..height {
        for [r, g, b, a] in rgba[y * stride..y * stride + row].as_chunks::<4>().0 {
            argb.extend_from_slice(&[*a, *r, *g, *b]);
        }
    }
    argb
}

/// The tray pictures decoded, for `MailTray::pixmaps`. Runs on the GTK
/// thread, where GDK decodes PNG itself.
pub fn pixmaps() -> Vec<ksni::Icon> {
    use gtk::gdk;
    use gtk::prelude::*;
    PIXMAPS
        .iter()
        .filter_map(|(_, png)| {
            let texture = crate::ui::texture::here(&gtk::glib::Bytes::from_static(png))?;
            let mut downloader = gdk::TextureDownloader::new(&texture);
            downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
            let (bytes, stride) = downloader.download_bytes();
            let (width, height) = (texture.width(), texture.height());
            let data = argb_from_rgba(width as usize, height as usize, stride, &bytes);
            (!data.is_empty()).then_some(ksni::Icon {
                width,
                height,
                data,
            })
        })
        .collect()
}

/// The badge's red, GNOME's red 3.
const BADGE: [f32; 3] = [224.0, 27.0, 36.0];

/// `icon` with a dot in its top right corner, which says mail is unread.
// shortcut: a dot, not the count; drawing digits needs a font, and the
// title and tooltip already give the number.
fn badged(icon: &ksni::Icon) -> ksni::Icon {
    let (width, height) = (icon.width as usize, icon.height as usize);
    let mut data = icon.data.clone();
    if data.len() < width * height * 4 {
        return icon.clone();
    }
    let radius = width as f32 * 0.2;
    let (cx, cy) = (width as f32 - radius, radius);
    for y in 0..height {
        for x in 0..width {
            let distance = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            // One pixel of soft edge.
            let cover = (radius - distance + 0.5).clamp(0.0, 1.0);
            if cover == 0.0 {
                continue;
            }
            let pixel = &mut data[(y * width + x) * 4..][..4];
            let under = f32::from(pixel[0]) / 255.0 * (1.0 - cover);
            let alpha = cover + under;
            for (channel, badge) in pixel[1..].iter_mut().zip(BADGE) {
                *channel = ((badge * cover + f32::from(*channel) * under) / alpha).round() as u8;
            }
            pixel[0] = (alpha * 255.0).round() as u8;
        }
    }
    ksni::Icon {
        width: icon.width,
        height: icon.height,
        data,
    }
}

pub struct MailTray {
    pub unread: i64,
    /// Each account's id, address and unread INBOX count.
    pub accounts: Vec<AccountUnread>,
    pub commands: async_channel::Sender<TrayCommand>,
    /// This copy can update itself, so the menu offers a check.
    pub can_update: bool,
    /// A newer release waiting to be installed.
    pub update: Option<String>,
    /// Where the installed icons are when the host may not search there;
    /// see `installed_icons`.
    pub icon_theme_path: String,
    /// The app icon; see `pixmaps`.
    pub pixmaps: Vec<ksni::Icon>,
    /// Show the app icon rather than the envelope glyph.
    pub penguin: bool,
}

/// The icon folder of the install this program runs from, for a host
/// that does not search it, or nothing. A snap's icons are not on the
/// host's search path, and without them the host draws a blank.
pub fn installed_icons() -> String {
    crate::exe::path()
        .ok()
        .and_then(|exe| icons_beside(&exe))
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// share/icons beside the bin folder holding `exe`, when the tray's
/// glyph is installed there.
fn icons_beside(exe: &std::path::Path) -> Option<std::path::PathBuf> {
    let icons = exe.parent()?.parent()?.join("share/icons");
    let glyph = format!("hicolor/symbolic/apps/{APP_ID}-unread-symbolic.svg");
    icons.join(glyph).is_file().then_some(icons)
}

/// The text beside the icon for `unread`: the count, held to four
/// characters, and nothing when nothing is unread.
fn label(unread: i64) -> String {
    match unread {
        ..=0 => String::new(),
        1..=999 => unread.to_string(),
        _ => "999+".into(),
    }
}

impl MailTray {
    fn send(&self, command: TrayCommand) {
        let _ = self.commands.try_send(command);
    }

    fn summary(&self) -> String {
        summary(self.unread)
    }
}

impl ksni::Tray for MailTray {
    fn id(&self) -> String {
        APP_ID.into()
    }

    fn title(&self) -> String {
        fill(
            &gettext("Penguin Mail: {summary}"),
            &[("summary", &self.summary())],
        )
    }

    fn icon_theme_path(&self) -> String {
        self.icon_theme_path.clone()
    }

    /// The app's own envelope glyph, drawn by the host in the panel's
    /// colour like its other icons. With mail unread it carries a dot,
    /// for the hosts that leave out the label. With no name, every host
    /// draws the penguin from the pixmaps.
    fn icon_name(&self) -> String {
        match (self.penguin, self.unread) {
            (true, _) => String::new(),
            (false, ..=0) => format!("{APP_ID}-symbolic"),
            (false, _) => format!("{APP_ID}-unread-symbolic"),
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        match (self.penguin, self.unread > 0) {
            (false, _) => Vec::new(),
            (true, true) => self.pixmaps.iter().map(badged).collect(),
            (true, false) => self.pixmaps.clone(),
        }
    }

    /// A host that shows the label reads it out as the item's name unless
    /// it has this, and a bare number says little.
    fn icon_accessible_desc(&self) -> String {
        self.title()
    }

    /// GNOME's AppIndicator extension and Ayatana panels draw this beside
    /// the icon at the panel's font size.
    fn x_ayatana_label(&self) -> String {
        label(self.unread)
    }

    fn x_ayatana_label_guide(&self) -> String {
        label(self.unread)
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: fill(
                &gettext("Penguin Mail: {summary}"),
                &[("summary", &self.summary())],
            ),
            description: self
                .accounts
                .iter()
                .filter(|a| a.unread > 0)
                .map(|a| format!("{}: {}", a.email, a.unread))
                .collect::<Vec<_>>()
                .join("\n"),
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayCommand::Toggle);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{MenuItem, StandardItem};
        let mut items: Vec<MenuItem<Self>> = account_lines(&self.accounts)
            .into_iter()
            .map(|line| match line {
                AccountLine::Unread { id, label } => StandardItem {
                    label,
                    activate: Box::new(move |tray: &mut Self| {
                        tray.send(TrayCommand::OpenInbox(id));
                    }),
                    ..Default::default()
                }
                .into(),
                AccountLine::NoUnread(label) => StandardItem {
                    label,
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            })
            .collect();
        if !items.is_empty() {
            items.push(MenuItem::Separator);
        }
        let item = |label: &str, command: fn() -> TrayCommand| -> MenuItem<Self> {
            StandardItem {
                label: label.into(),
                activate: Box::new(move |tray: &mut Self| tray.send(command())),
                ..Default::default()
            }
            .into()
        };
        items.push(item(&gettext("Open Penguin Mail"), || TrayCommand::Open));
        items.push(item(&gettext("New Message"), || TrayCommand::Compose));
        items.push(item(&gettext("Check for Mail"), || TrayCommand::Check));
        if let Some(version) = &self.update {
            items.push(MenuItem::Separator);
            let install = fill(
                &gettext("Install Update {version}"),
                &[("version", version)],
            );
            let notes = fill(&gettext("What's New in {version}"), &[("version", version)]);
            items.push(item(&install, || TrayCommand::InstallUpdate));
            items.push(item(&notes, || TrayCommand::WhatsNew));
        } else if self.can_update {
            items.push(item(&gettext("Check for Updates"), || {
                TrayCommand::CheckForUpdates
            }));
        }
        items.push(MenuItem::Separator);
        items.push(item(&gettext("Quit"), || TrayCommand::Quit));
        items
    }
}

/// Checks that need GTK, run from the one GTK test in
/// `ui::composer::richbuffer`.
#[cfg(test)]
pub mod checks {
    pub fn run() {
        the_tray_pictures_decode_to_argb();
    }

    fn the_tray_pictures_decode_to_argb() {
        let icons = super::pixmaps();
        let sizes: Vec<i32> = icons.iter().map(|icon| icon.width).collect();
        assert_eq!(sizes, [16, 22, 24, 32, 48]);
        for icon in &icons {
            let (w, h) = (icon.width as usize, icon.height as usize);
            assert_eq!(icon.data.len(), w * h * 4);
            // The middle of the icon is the penguin's face: opaque paper,
            // alpha first.
            let middle = ((h / 2) * w + w / 2) * 4;
            assert_eq!(icon.data[middle], 255, "{w} px icon is opaque in the middle");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AccountId, AccountLine, AccountUnread, account_line, account_lines, summary};

    fn tray(unread: i64) -> super::MailTray {
        let (commands, _received) = async_channel::unbounded();
        super::MailTray {
            unread,
            accounts: Vec::new(),
            commands,
            can_update: false,
            update: None,
            icon_theme_path: String::new(),
            pixmaps: Vec::new(),
            penguin: false,
        }
    }

    #[test]
    fn a_pixmap_turns_rgba_rows_into_argb_and_drops_the_padding() {
        // Two pixels a row, then two bytes of padding to a stride of 10.
        let rgba = [
            1, 2, 3, 4, 5, 6, 7, 8, 0, 0, //
            9, 10, 11, 12, 13, 14, 15, 16, 0, 0,
        ];
        assert_eq!(
            super::argb_from_rgba(2, 2, 10, &rgba),
            [4, 1, 2, 3, 8, 5, 6, 7, 12, 9, 10, 11, 16, 13, 14, 15]
        );
    }

    #[test]
    fn unread_mail_puts_a_dot_in_the_top_right_corner() {
        let blank = ksni::Icon {
            width: 22,
            height: 22,
            data: [255, 10, 20, 30].repeat(22 * 22),
        };
        let badged = super::badged(&blank);
        assert_eq!(badged.data.len(), blank.data.len());
        let at = |x: usize, y: usize| badged.data[(y * 22 + x) * 4..][..4].to_vec();
        assert_eq!(at(16, 5), [255, 224, 27, 36], "the middle of the dot");
        assert_eq!(at(2, 19), [255, 10, 20, 30], "the far corner");
        let clear = ksni::Icon {
            data: vec![0; 22 * 22 * 4],
            ..blank
        };
        let dot = super::badged(&clear).data;
        assert_eq!(dot[(5 * 22 + 16) * 4..][..4], [255, 224, 27, 36]);
        assert_eq!(dot[(19 * 22 + 2) * 4], 0, "nothing drawn away from the dot");
    }

    #[test]
    fn a_pixmap_short_of_rows_is_none() {
        assert_eq!(super::argb_from_rgba(2, 2, 8, &[0; 12]), Vec::<u8>::new());
    }

    #[test]
    fn each_tray_picture_is_the_size_it_is_sent_as() {
        for (size, png) in super::PIXMAPS {
            // A PNG's width and height are the two big-endian words after
            // the signature and the IHDR chunk's length and name.
            let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
            let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
            assert_eq!((width, height), (size, size));
        }
    }

    #[test]
    fn the_penguin_names_no_icon_and_carries_the_dot_while_mail_is_unread() {
        use ksni::Tray;
        let mut tray = tray(0);
        tray.penguin = true;
        tray.pixmaps = vec![ksni::Icon {
            width: 22,
            height: 22,
            data: vec![0; 22 * 22 * 4],
        }];
        assert_eq!(tray.icon_name(), "");
        assert_eq!(tray.icon_pixmap()[0].data, tray.pixmaps[0].data);
        tray.unread = 2;
        assert_eq!(tray.icon_name(), "");
        assert_ne!(tray.icon_pixmap()[0].data, tray.pixmaps[0].data);
        assert_eq!(tray.x_ayatana_label(), "2");
    }

    #[test]
    fn with_nothing_unread_the_tray_shows_the_plain_envelope_and_no_label() {
        use ksni::Tray;
        let tray = tray(0);
        assert_eq!(tray.icon_name(), format!("{}-symbolic", crate::APP_ID));
        assert_eq!(tray.x_ayatana_label(), "");
        assert_eq!(tray.x_ayatana_label_guide(), "");
        assert_eq!(tray.tool_tip().title, "Penguin Mail: No unread mail");
        assert_eq!(tray.icon_accessible_desc(), "Penguin Mail: No unread mail");
        assert!(tray.icon_pixmap().is_empty());
    }

    #[test]
    fn one_unread_message_puts_a_dot_on_the_envelope_and_1_beside_it() {
        use ksni::Tray;
        let tray = tray(1);
        assert_eq!(
            tray.icon_name(),
            format!("{}-unread-symbolic", crate::APP_ID)
        );
        assert_eq!(tray.x_ayatana_label(), "1");
        assert_eq!(tray.x_ayatana_label_guide(), "1");
        assert_eq!(tray.tool_tip().title, "Penguin Mail: 1 unread message");
        assert_eq!(
            tray.icon_accessible_desc(),
            "Penguin Mail: 1 unread message"
        );
        assert!(tray.icon_pixmap().is_empty());
    }

    #[test]
    fn a_large_count_is_written_out_in_full() {
        use ksni::Tray;
        let tray = tray(120);
        assert_eq!(
            tray.icon_name(),
            format!("{}-unread-symbolic", crate::APP_ID)
        );
        assert_eq!(tray.x_ayatana_label(), "120");
        assert_eq!(tray.tool_tip().title, "Penguin Mail: 120 unread messages");
    }

    /// Reading everything takes the count off the panel: the label goes
    /// empty and the icon and tooltip are the ones with nothing unread.
    #[test]
    fn going_from_3_unread_to_none_clears_the_label_the_dot_and_the_count() {
        use ksni::Tray;
        let mut tray = tray(3);
        tray.accounts = vec![unread(1, "a@example.com", 3)];
        assert_eq!(tray.x_ayatana_label(), "3");
        assert_eq!(tray.tool_tip().description, "a@example.com: 3");
        tray.unread = 0;
        tray.accounts = vec![unread(1, "a@example.com", 0)];
        assert_eq!(tray.x_ayatana_label(), "");
        assert_eq!(tray.x_ayatana_label_guide(), "");
        assert_eq!(tray.icon_name(), format!("{}-symbolic", crate::APP_ID));
        assert_eq!(tray.tool_tip().title, "Penguin Mail: No unread mail");
        assert_eq!(tray.tool_tip().description, "");
        assert_eq!(tray.icon_accessible_desc(), "Penguin Mail: No unread mail");
    }

    /// Past 999 the label stops growing; the tooltip keeps the number.
    #[test]
    fn past_999_the_label_says_999_plus() {
        use ksni::Tray;
        let tray = tray(4321);
        assert_eq!(tray.x_ayatana_label(), "999+");
        assert_eq!(tray.tool_tip().title, "Penguin Mail: 4321 unread messages");
    }

    /// Both glyphs ship with the app, under the names the tray sends.
    #[test]
    fn the_tray_glyphs_are_in_the_icon_folder() {
        let apps =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/icons/scalable/apps");
        for name in [
            "io.github.c9dev.PenguinMail-symbolic",
            "io.github.c9dev.PenguinMail-unread-symbolic",
        ] {
            assert!(apps.join(format!("{name}.svg")).is_file(), "{name}");
        }
    }

    /// An install's icons sit in share/icons beside the bin folder that
    /// holds the program. A snap's are not on the host's search path, so
    /// the tray names the folder; a build run from target/ has none.
    #[test]
    fn the_icon_folder_is_found_beside_the_program() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("bin/penguin-mail");
        assert_eq!(super::icons_beside(&exe), None);
        let glyph = root
            .path()
            .join("share/icons/hicolor/symbolic/apps")
            .join(format!("{}-unread-symbolic.svg", crate::APP_ID));
        std::fs::create_dir_all(glyph.parent().unwrap()).unwrap();
        std::fs::write(&glyph, "<svg/>").unwrap();
        assert_eq!(
            super::icons_beside(&exe),
            Some(root.path().join("share/icons"))
        );
    }

    #[test]
    fn the_tray_counts_unread_mail_in_a_sentence() {
        assert_eq!(summary(0), "No unread mail");
        assert_eq!(summary(1), "1 unread message");
        assert_eq!(summary(7), "7 unread messages");
    }

    #[test]
    fn an_account_line_says_what_its_number_counts() {
        assert_eq!(account_line("ann@example.com", 0), "ann@example.com");
        assert_eq!(
            account_line("ann@example.com", 1),
            "ann@example.com, 1 unread message"
        );
        assert_eq!(
            account_line("ann@example.com", 3),
            "ann@example.com, 3 unread messages"
        );
    }

    fn unread(id: AccountId, email: &str, unread: i64) -> AccountUnread {
        AccountUnread {
            id,
            email: email.into(),
            unread,
        }
    }

    #[test]
    fn the_menu_lists_only_accounts_with_unread_mail_in_their_order() {
        let lines = account_lines(&[
            unread(1, "a@example.com", 0),
            unread(2, "b@example.com", 4),
            unread(3, "c@example.com", 0),
            unread(4, "d@example.com", 1),
        ]);
        assert_eq!(
            lines,
            [
                AccountLine::Unread {
                    id: 2,
                    label: "b@example.com, 4 unread messages".into()
                },
                AccountLine::Unread {
                    id: 4,
                    label: "d@example.com, 1 unread message".into()
                },
            ]
        );
    }

    #[test]
    fn the_menu_says_so_when_nothing_is_unread() {
        let lines = account_lines(&[unread(1, "a@example.com", 0), unread(2, "b@example.com", 0)]);
        assert_eq!(lines, [AccountLine::NoUnread("No unread mail".into())]);
    }

    #[test]
    fn the_menu_lists_nothing_without_accounts() {
        assert_eq!(account_lines(&[]), []);
    }

    #[test]
    fn choosing_an_account_line_asks_to_open_that_inbox() {
        use ksni::Tray;
        use ksni::menu::MenuItem;
        let (commands, received) = async_channel::unbounded();
        let mut tray = super::MailTray {
            unread: 4,
            accounts: vec![unread(1, "a@example.com", 0), unread(2, "b@example.com", 4)],
            commands,
            can_update: false,
            update: None,
            icon_theme_path: String::new(),
            pixmaps: Vec::new(),
            penguin: false,
        };
        let menu = tray.menu();
        let MenuItem::Standard(line) = &menu[0] else {
            panic!("the first item is an account line");
        };
        assert_eq!(line.label, "b@example.com, 4 unread messages");
        (line.activate)(&mut tray);
        assert!(matches!(
            received.try_recv(),
            Ok(super::TrayCommand::OpenInbox(2))
        ));
        assert!(matches!(menu[1], MenuItem::Separator));
    }
}
