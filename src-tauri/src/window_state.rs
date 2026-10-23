//! Remember the main window without writing during every drag/resize event.
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Window, WindowEvent};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
struct Geometry {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

#[derive(Default)]
struct WindowState(Mutex<Option<Geometry>>);

fn fit(mut saved: Geometry, screens: &[(i32, i32, u32, u32)]) -> Option<Geometry> {
    if saved.width < 100 || saved.height < 100 || screens.is_empty() {
        return None;
    }
    let screen = screens
        .iter()
        .find(|&&(x, y, w, h)| {
            i64::from(saved.x) + i64::from(saved.width) > i64::from(x)
                && i64::from(saved.y) + i64::from(saved.height) > i64::from(y)
                && i64::from(saved.x) < i64::from(x) + i64::from(w)
                && i64::from(saved.y) < i64::from(y) + i64::from(h)
        })
        .unwrap_or(&screens[0]);
    let &(x, y, width, height) = screen;
    if width < 100 || height < 100 {
        return None;
    }
    saved.width = saved.width.max(640).min(width);
    saved.height = saved.height.max(500).min(height);
    saved.x = i64::from(saved.x).clamp(i64::from(x), i64::from(x) + i64::from(width - saved.width))
        as i32;
    saved.y = i64::from(saved.y).clamp(
        i64::from(y),
        i64::from(y) + i64::from(height - saved.height),
    ) as i32;
    Some(saved)
}

pub(crate) fn restore(app: &AppHandle) {
    app.manage(WindowState::default());
    let Some(webview_window) = app.get_webview_window("main") else {
        return;
    };
    let window = webview_window.as_ref().window();
    let saved = crate::app_paths::app_data_dir(app)
        .ok()
        .and_then(|dir| std::fs::read(dir.join("window-state.json")).ok())
        .and_then(|bytes| serde_json::from_slice::<Geometry>(&bytes).ok());
    let screens = window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            (position.x, position.y, size.width, size.height)
        })
        .collect::<Vec<_>>();
    if let Some(saved) = saved.and_then(|saved| fit(saved, &screens)) {
        let _ = window.set_size(PhysicalSize::new(saved.width, saved.height));
        let _ = window.set_position(PhysicalPosition::new(saved.x, saved.y));
        *app.state::<WindowState>()
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(saved);
        if saved.maximized {
            let _ = window.maximize();
        }
    } else {
        capture(&window);
    }
}

fn capture(window: &Window) {
    let Some(state) = window.app_handle().try_state::<WindowState>() else {
        return;
    };
    if window.is_minimized().unwrap_or(true) || window.is_fullscreen().unwrap_or(true) {
        return;
    }
    let maximized = window.is_maximized().unwrap_or(false);
    let mut saved = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if maximized {
        if let Some(saved) = saved.as_mut() {
            saved.maximized = true;
        }
        return;
    }
    if let (Ok(position), Ok(size)) = (window.outer_position(), window.inner_size()) {
        if size.width >= 100 && size.height >= 100 {
            *saved = Some(Geometry {
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
                maximized,
            });
        }
    }
}

pub(crate) fn save(app: &AppHandle) {
    let Some(state) = app.try_state::<WindowState>() else {
        return;
    };
    let saved = *state.0.lock().unwrap_or_else(|e| e.into_inner());
    let result = (|| -> Result<(), String> {
        let Some(saved) = saved else {
            return Ok(());
        };
        let dir = crate::app_paths::app_data_dir(app)?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(
            dir.join("window-state.json"),
            serde_json::to_vec(&saved).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    })();
    if let Err(error) = result {
        log::warn!("保存窗口位置失败: {error}");
    }
}

pub(crate) fn on_event(window: &Window, event: &WindowEvent) {
    if window.label() != "main" {
        return;
    }
    match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => capture(window),
        WindowEvent::CloseRequested { .. } | WindowEvent::Focused(false) => {
            capture(window);
            save(window.app_handle());
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn geometry() -> Geometry {
        Geometry {
            x: 100,
            y: 100,
            width: 900,
            height: 600,
            maximized: false,
        }
    }
    #[test]
    fn normal_geometry_survives_round_trip() {
        let saved = geometry();
        assert_eq!(
            serde_json::from_slice::<Geometry>(&serde_json::to_vec(&saved).unwrap()).unwrap(),
            saved
        );
        assert_eq!(fit(saved, &[(0, 0, 1920, 1080)]), Some(saved));
    }
    #[test]
    fn disconnected_monitor_and_oversized_window_move_onscreen() {
        let saved = Geometry {
            x: -9000,
            y: 9000,
            width: 4000,
            height: 3000,
            maximized: true,
        };
        assert_eq!(
            fit(saved, &[(0, 0, 1920, 1080)]),
            Some(Geometry {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                maximized: true
            })
        );
    }
    #[test]
    fn negative_monitor_coordinates_are_valid() {
        let saved = Geometry {
            x: -1800,
            ..geometry()
        };
        assert_eq!(
            fit(saved, &[(0, 0, 1920, 1080), (-1920, 0, 1920, 1080)]),
            Some(saved)
        );
    }
    #[test]
    fn invalid_geometry_and_missing_monitors_are_ignored() {
        assert!(fit(
            Geometry {
                width: 0,
                ..geometry()
            },
            &[(0, 0, 1920, 1080)]
        )
        .is_none());
        assert!(fit(geometry(), &[]).is_none());
    }
}
