use tauri::image::Image;
use windows::core::{w, PCWSTR};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SM_CXSMICON};

use crate::models::WindowsTrayIconStyle;
use crate::tray_visual::{
    codex_tools_icon_rgba as render_codex_tools_icon_rgba, render_tray_visual,
    tray_visual_dimensions, TrayVisualPlatform, TrayVisualStatus,
};
use crate::windows_taskbar_widget::{system_uses_light_theme, WindowsWidgetStatus};

const FALLBACK_ICON_SIZE: u32 = 24;

pub(crate) fn static_codex_tools_icon() -> Image<'static> {
    let size = windows_tray_icon_size();
    Image::new_owned(codex_tools_icon_rgba(size), size, size)
}

pub(crate) fn codex_tools_icon_rgba(destination_size: u32) -> Vec<u8> {
    render_codex_tools_icon_rgba(destination_size)
}

pub(crate) fn render_windows_tray_icon(
    style: WindowsTrayIconStyle,
    percent: Option<f64>,
    status: WindowsWidgetStatus,
) -> Image<'static> {
    let size = windows_tray_icon_size();
    let (width, height) = tray_visual_dimensions(style, TrayVisualPlatform::Windows, size);
    render_tray_visual(
        style,
        percent,
        match status {
            WindowsWidgetStatus::Fresh => TrayVisualStatus::Fresh,
            WindowsWidgetStatus::Stale => TrayVisualStatus::Stale,
            WindowsWidgetStatus::Error => TrayVisualStatus::Error,
            WindowsWidgetStatus::Unavailable => TrayVisualStatus::Unavailable,
        },
        system_uses_light_theme(),
        width,
        height,
    )
}

pub(crate) fn windows_tray_icon_size() -> u32 {
    unsafe {
        let taskbar = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()).ok();
        let dpi = taskbar
            .filter(|handle| !handle.is_invalid())
            .map(|handle| GetDpiForWindow(handle))
            .unwrap_or(144)
            .max(96);
        GetSystemMetricsForDpi(SM_CXSMICON, dpi)
            .try_into()
            .ok()
            .filter(|size: &u32| (16..=64).contains(size))
            .unwrap_or(FALLBACK_ICON_SIZE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_notification_icon_size_is_sane() {
        assert!((16..=64).contains(&windows_tray_icon_size()));
    }

    #[test]
    fn renders_owned_rgba_for_every_style() {
        let styles = [
            WindowsTrayIconStyle::GradientNumberPlate,
            WindowsTrayIconStyle::GradientNumberCard,
            WindowsTrayIconStyle::GradientNumber,
            WindowsTrayIconStyle::NumberProgressBar,
            WindowsTrayIconStyle::LogoProgressRing,
        ];
        for style in styles {
            let image = render_windows_tray_icon(style, Some(37.4), WindowsWidgetStatus::Fresh);
            assert_eq!(image.width(), image.height());
            assert!((16..=64).contains(&image.width()));
            assert_eq!(
                image.rgba().len(),
                (image.width() * image.height() * 4) as usize
            );
            assert!(image.rgba().chunks_exact(4).any(|pixel| pixel[3] > 0));
        }
    }

    #[test]
    fn error_keeps_the_cached_visual_and_missing_values_use_a_placeholder() {
        for style in [
            WindowsTrayIconStyle::GradientNumberPlate,
            WindowsTrayIconStyle::GradientNumberCard,
            WindowsTrayIconStyle::GradientNumber,
            WindowsTrayIconStyle::NumberProgressBar,
            WindowsTrayIconStyle::LogoProgressRing,
        ] {
            let fresh = render_windows_tray_icon(style, Some(50.0), WindowsWidgetStatus::Fresh);
            let error = render_windows_tray_icon(style, Some(50.0), WindowsWidgetStatus::Error);
            assert_eq!(error.rgba(), fresh.rgba(), "style={style:?}");
        }

        let missing = render_windows_tray_icon(
            WindowsTrayIconStyle::GradientNumberPlate,
            None,
            WindowsWidgetStatus::Unavailable,
        );
        let cached = render_windows_tray_icon(
            WindowsTrayIconStyle::GradientNumberPlate,
            Some(50.0),
            WindowsWidgetStatus::Error,
        );
        assert_ne!(cached.rgba(), missing.rgba());
    }
}
