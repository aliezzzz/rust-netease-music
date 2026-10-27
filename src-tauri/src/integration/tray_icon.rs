//! Embedded tray-only artwork; independent of the application/Dock icon.
//! Generated from icons/tray/music.svg using the Tauri CLI (see its README).

#[cfg(target_os = "linux")]
pub(super) fn linux_icons() -> tauri::Result<Vec<ksni::Icon>> {
    [
        include_bytes!("../../icons/tray/16x16.png").as_slice(),
        include_bytes!("../../icons/tray/22x22.png").as_slice(),
        include_bytes!("../../icons/tray/24x24.png").as_slice(),
        include_bytes!("../../icons/tray/32x32.png").as_slice(),
        include_bytes!("../../icons/tray/48x48.png").as_slice(),
        include_bytes!("../../icons/tray/64x64.png").as_slice(),
    ]
    .into_iter()
    .map(|png| {
        let image = tauri::image::Image::from_bytes(png)?;
        Ok(ksni::Icon {
            width: image.width() as i32,
            height: image.height() as i32,
            data: rgba_to_argb(image.rgba()),
        })
    })
    .collect()
}

#[cfg(target_os = "linux")]
fn rgba_to_argb(rgba: &[u8]) -> Vec<u8> {
    // StatusNotifierItem expects ARGB in network byte order, not native u32s.
    rgba.chunks_exact(4)
        .flat_map(|p| [p[3], p[0], p[1], p[2]])
        .collect()
}

#[cfg(target_os = "windows")]
pub(super) fn windows_icon() -> tauri::Result<tauri::image::Image<'static>> {
    tauri::image::Image::from_bytes(include_bytes!("../../icons/tray/32x32.png"))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn pixel_order_preserves_alpha_and_color() {
        assert_eq!(rgba_to_argb(&[10, 20, 30, 40]), [40, 10, 20, 30]);
    }

    #[test]
    fn tray_assets_are_white_notes_on_transparency() {
        let icons = linux_icons().unwrap();
        assert_eq!(
            icons.iter().map(|i| i.width).collect::<Vec<_>>(),
            [16, 22, 24, 32, 48, 64]
        );
        for icon in icons {
            assert_eq!(icon.width, icon.height);
            assert_eq!(icon.data.len(), (icon.width * icon.height * 4) as usize);
            assert_eq!(icon.data[0], 0, "corner must be transparent");
            assert!(icon.data.chunks_exact(4).any(|p| p[0] == 255));
            assert!(
                icon.data.chunks_exact(4).any(|p| p[0] > 0 && p[0] < 255),
                "antialiased edges"
            );
            assert!(icon
                .data
                .chunks_exact(4)
                .filter(|p| p[0] > 0)
                .all(|p| p[1..] == [255, 255, 255]));
        }
    }
}
