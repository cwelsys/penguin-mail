//! The penguin the tray draws from pixmaps when the `tray_penguin`
//! preference is on.

/// The app icon at the sizes trays draw, for a host that cannot find the
/// icon by name. A snap's or Flatpak's icons are not on the host, and
/// Cinnamon's tray shows a blank picture when neither the name nor a
/// pixmap gives it one. The colour icon reads on light and dark panels,
/// where a host that does not recolor symbolic icons leaves the glyph dark.
const PIXMAPS: [(u32, &[u8]); 5] = [
    (16, include_bytes!("../../data/tray/16.png")),
    (22, include_bytes!("../../data/tray/22.png")),
    (24, include_bytes!("../../data/tray/24.png")),
    (32, include_bytes!("../../data/tray/32.png")),
    (48, include_bytes!("../../data/tray/48.png")),
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
pub(super) fn badged(icon: &ksni::Icon) -> ksni::Icon {
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
            assert_eq!(
                icon.data[middle], 255,
                "{w} px icon is opaque in the middle"
            );
        }
    }
}

#[cfg(test)]
mod tests {
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
}
