use super::{
    base_height_for_text, blend_text_mask, desired_size, embedded_screen_position,
    left_screen_position, measure_system_title_text, pixels_are_premultiplied_bgra,
    rasterize_system_title_text_mask, rect_covers_monitor, render_widget_pixels,
    resolved_system_title_font_face, scale, should_hide_for_fullscreen, taskbar_edge,
    visible_taskbar_thickness, widget_foreground, widget_text_lines, TaskbarEdge, TaskbarPlacement,
    WindowsWidgetStatus, BASE_ICON_GAP, BASE_ICON_SIZE, BASE_PADDING, BASE_SINGLE_LINE_HEIGHT,
    SYSTEM_TITLE_FONT_FALLBACK, SYSTEM_TITLE_FONT_PRIMARY,
};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::MONITORINFO;
use windows::Win32::UI::WindowsAndMessaging::WS_MAXIMIZE;

#[test]
fn native_state_policy_keeps_hide_and_geometry_requests_intact() {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::HWND_BOTTOM;
    for flags in [SWP_HIDEWINDOW, SWP_NOZORDER] {
        let mut position = WINDOWPOS {
            hwndInsertAfter: HWND_BOTTOM,
            flags,
            x: 15,
            y: -4,
            cx: 80,
            cy: 24,
            ..Default::default()
        };
        unsafe {
            preserve_widget_window_state(
                HWND::default(),
                WM_WINDOWPOSCHANGING,
                WPARAM(0),
                LPARAM((&mut position as *mut WINDOWPOS) as isize),
            );
        }
        assert_eq!(position.hwndInsertAfter, HWND_BOTTOM);
        assert_eq!(position.flags, flags);
        assert_eq!(
            (position.x, position.y, position.cx, position.cy),
            (15, -4, 80, 24)
        );
    }
}

#[test]
fn native_state_policy_blocks_demotion_before_it_takes_effect() {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{
        DestroyWindow, UnregisterClassW, HWND_BOTTOM, HWND_NOTOPMOST,
    };
    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if preserve_widget_window_state(hwnd, msg, wp, lp) {
            LRESULT(0)
        } else {
            DefWindowProcW(hwnd, msg, wp, lp)
        }
    }
    struct TestWindow(HWND, HINSTANCE);
    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
                let _ = UnregisterClassW(w!("CodexTaskbarStateTest"), Some(self.1));
            }
        }
    }
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: w!("CodexTaskbarStateTest"),
            ..Default::default()
        };
        assert_ne!(RegisterClassExW(&class), 0);
        let window = TestWindow(
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
                w!("CodexTaskbarStateTest"),
                w!("Quota state test"),
                WS_POPUP,
                -31000,
                -31000,
                80,
                24,
                None,
                None,
                Some(instance),
                None,
            )
            .unwrap(),
            instance,
        );
        let foreground = GetForegroundWindow();
        let _ = ShowWindow(window.0, SW_SHOWNOACTIVATE);
        for after in [HWND_NOTOPMOST, HWND_BOTTOM] {
            SetWindowPos(
                window.0,
                Some(after),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            )
            .unwrap();
            assert_ne!(
                GetWindowLongPtrW(window.0, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST.0,
                0
            );
            assert_eq!(GetForegroundWindow(), foreground);
        }
        SetWindowLongPtrW(window.0, GWL_EXSTYLE, 0);
        let style = GetWindowLongPtrW(window.0, GWL_EXSTYLE) as u32;
        let required = (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED).0;
        assert_eq!(style & required, required);
        restore_widget_z_order(window.0);
        assert_ne!(
            GetWindowLongPtrW(window.0, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST.0,
            0
        );
        let _ = ShowWindow(window.0, SW_HIDE);
        assert!(!IsWindowVisible(window.0).as_bool());
        let _ = ShowWindow(window.0, SW_SHOWNOACTIVATE);
        assert!(IsWindowVisible(window.0).as_bool());
        assert_eq!(GetForegroundWindow(), foreground);
        let mut rect = RECT::default();
        GetWindowRect(window.0, &mut rect).unwrap();
        assert_eq!(
            (rect.left, rect.top, rect.right, rect.bottom),
            (-31000, -31000, -30920, -30976)
        );
    }
}

#[test]
fn z_order_guard_repairs_repeated_occlusion_without_activation_or_movement() {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::DestroyWindow;

    struct TestWindow(HWND);
    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }
    unsafe fn create(owner: Option<HWND>) -> TestWindow {
        TestWindow(
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                w!("STATIC"),
                w!("Quota Z-order test"),
                WS_POPUP,
                -30000,
                -30000,
                80,
                24,
                owner,
                None,
                None,
                None,
            )
            .unwrap(),
        )
    }
    unsafe {
        // Off-screen windows exercise User32 without covering the user's desktop.
        let widget = create(None);
        let blocker = create(None);
        let foreground = GetForegroundWindow();
        let _ = ShowWindow(widget.0, SW_SHOWNOACTIVATE);
        for _ in 0..12 {
            SetWindowPos(
                blocker.0,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            )
            .unwrap();
            assert!(covering_window_above(widget.0));
            assert!(restore_widget_z_order(widget.0));
            assert!(!covering_window_above(widget.0));
            assert!(!restore_widget_z_order(widget.0));
            assert_eq!(GetForegroundWindow(), foreground);
            let mut rect = RECT::default();
            GetWindowRect(widget.0, &mut rect).unwrap();
            assert_eq!(
                (rect.left, rect.top, rect.right, rect.bottom),
                (-30000, -30000, -29920, -29976)
            );
        }
        let _ = ShowWindow(blocker.0, SW_HIDE);
        let tooltip = create(Some(widget.0));
        let _ = ShowWindow(tooltip.0, SW_SHOWNOACTIVATE);
        assert!(
            !restore_widget_z_order(widget.0),
            "owned tooltips stay above the widget"
        );
    }
}

#[test]
fn z_order_guard_ignores_nonoverlapping_windows() {
    let widget = RECT {
        left: 10,
        top: 10,
        right: 90,
        bottom: 34,
    };
    assert!(super::rects_overlap(
        widget,
        RECT {
            left: 20,
            top: 20,
            right: 100,
            bottom: 40
        }
    ));
    assert!(!super::rects_overlap(
        widget,
        RECT {
            left: 90,
            top: 10,
            right: 170,
            bottom: 34
        }
    ));
    assert!(!super::rects_overlap(
        widget,
        RECT {
            left: 10,
            top: 34,
            right: 90,
            bottom: 58
        }
    ));
}

#[test]
fn shell_events_refresh_layout_without_following_unrelated_controls() {
    use super::*;
    assert!(event_needs_layout(
        EVENT_SYSTEM_FOREGROUND,
        0,
        false,
        false,
        false
    ));
    assert!(event_needs_layout(
        EVENT_OBJECT_REORDER,
        OBJID_WINDOW.0,
        true,
        false,
        false
    ));
    assert!(event_needs_layout(
        EVENT_OBJECT_REORDER,
        OBJID_CLIENT.0,
        false,
        true,
        false
    ));
    assert!(event_needs_layout(
        EVENT_OBJECT_LOCATIONCHANGE,
        OBJID_WINDOW.0,
        true,
        false,
        false
    ));
    assert!(event_needs_layout(
        EVENT_OBJECT_LOCATIONCHANGE,
        OBJID_WINDOW.0,
        false,
        false,
        true
    ));
    assert!(!event_needs_layout(
        EVENT_OBJECT_REORDER,
        OBJID_WINDOW.0,
        false,
        false,
        true
    ));
    assert!(!event_needs_layout(
        EVENT_OBJECT_LOCATIONCHANGE,
        OBJID_CLIENT.0,
        false,
        false,
        true
    ));
}

#[test]
fn desktop_surfaces_are_not_fullscreen_apps() {
    for class in ["Progman", "WorkerW", "Shell_TrayWnd"] {
        assert!(super::is_desktop_surface(class));
    }
    assert!(!super::is_desktop_surface("Chrome_WidgetWin_1"));
    assert!(!super::is_desktop_surface("Windows.UI.Core.CoreWindow"));
}

#[test]
fn embedded_window_follows_its_parent_without_z_order_repair() {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{DestroyWindow, IsWindow, UnregisterClassW};
    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if preserve_widget_window_state(hwnd, msg, wp, lp) {
            LRESULT(0)
        } else {
            DefWindowProcW(hwnd, msg, wp, lp)
        }
    }
    struct Window(HWND);
    impl Drop for Window {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: w!("CodexEmbeddedQuotaTest"),
            ..Default::default()
        };
        assert_ne!(RegisterClassExW(&class), 0);
        let parent = Window(
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!("Test taskbar"),
                WS_POPUP,
                -28000,
                -28000,
                400,
                48,
                None,
                None,
                Some(instance),
                None,
            )
            .unwrap(),
        );
        let child = Window(
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
                w!("CodexEmbeddedQuotaTest"),
                w!("Test quota"),
                WS_POPUP,
                0,
                0,
                80,
                24,
                None,
                None,
                Some(instance),
                None,
            )
            .unwrap(),
        );
        let foreground = GetForegroundWindow();
        assert!(set_widget_parent(child.0, Some(parent.0)));
        assert_eq!(GetParent(child.0).unwrap(), parent.0);
        assert!(is_taskbar_child(child.0));
        assert_eq!(GetWindowLongPtrW(child.0, GWL_STYLE) as u32 & WS_POPUP.0, 0);
        SetWindowPos(
            child.0,
            Some(HWND_TOP),
            20,
            4,
            80,
            24,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
        .unwrap();
        for i in 0..12 {
            // No layout/event callback or timer runs between the parent's raise
            // and these checks: the child follows as part of the window tree.
            SetWindowPos(
                parent.0,
                Some(HWND_TOPMOST),
                -28000 + i,
                -28000,
                400,
                48,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
            .unwrap();
            assert_eq!(GetParent(child.0).unwrap(), parent.0);
            assert!(IsWindowVisible(child.0).as_bool());
            let mut rect = RECT::default();
            GetWindowRect(child.0, &mut rect).unwrap();
            assert_eq!((rect.left, rect.top), (-27980 + i, -27996));
            let mut local = POINT {
                x: rect.left,
                y: rect.top,
            };
            assert!(ScreenToClient(parent.0, &mut local).as_bool());
            assert_eq!((local.x, local.y), (20, 4));
            assert_eq!(GetForegroundWindow(), foreground);
        }
        let _ = ShowWindow(child.0, SW_HIDE);
        assert!(!IsWindowVisible(child.0).as_bool());
        let _ = ShowWindow(child.0, SW_SHOWNOACTIVATE);
        assert!(IsWindowVisible(child.0).as_bool());
        assert!(set_widget_parent(child.0, None));
        assert!(!is_taskbar_child(child.0));
        assert!(GetParent(child.0).is_err());
        assert!(set_widget_parent(child.0, Some(parent.0)));
        DestroyWindow(parent.0).unwrap();
        assert!(!IsWindow(Some(child.0)).as_bool());
        drop(child);
        UnregisterClassW(w!("CodexEmbeddedQuotaTest"), Some(instance)).unwrap();
    }
}

#[test]
fn taskbar_lookup_is_pinned_to_the_primary_shell_window() {
    let source = include_str!("../windows_taskbar_widget.rs");
    let primary_lookup = ["FindWindowW(w!(\"", "Shell_TrayWnd", "\")"].concat();
    let secondary_class = ["Shell_", "SecondaryTrayWnd"].concat();
    let anchor_monitor_lookup = ["MonitorFromWindow(", "anchor_hwnd"].concat();

    assert!(source.contains(&primary_lookup));
    assert!(!source.contains(&secondary_class));
    assert!(!source.contains(&anchor_monitor_lookup));
}

#[test]
fn fullscreen_detection_requires_the_foreground_rect_to_cover_the_monitor() {
    let monitor = RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    assert!(rect_covers_monitor(monitor, monitor, 2));
    assert!(rect_covers_monitor(
        RECT {
            left: -1,
            top: -1,
            right: 1921,
            bottom: 1081,
        },
        monitor,
        2,
    ));
    assert!(!rect_covers_monitor(
        RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        },
        monitor,
        2,
    ));
    assert!(should_hide_for_fullscreen(None, 0, monitor, monitor));
    assert!(!should_hide_for_fullscreen(
        None,
        WS_MAXIMIZE.0,
        monitor,
        monitor,
    ));
    assert!(should_hide_for_fullscreen(
        Some(super::QUNS_RUNNING_D3D_FULL_SCREEN),
        WS_MAXIMIZE.0,
        RECT {
            left: 100,
            top: 100,
            right: 900,
            bottom: 700,
        },
        monitor,
    ));
}

#[test]
fn start_menu_busy_transition_does_not_hide_the_widget() {
    use windows::Win32::UI::Shell::{QUNS_ACCEPTS_NOTIFICATIONS, QUNS_APP, QUNS_BUSY};
    let monitor = RECT {
        left: 0,
        top: 0,
        right: 2560,
        bottom: 1440,
    };
    // Geometry captured while opening Start on Windows 11.
    let start_menu = RECT {
        left: 851,
        top: 502,
        right: 1709,
        bottom: 1392,
    };
    for state in [QUNS_BUSY, QUNS_APP, QUNS_ACCEPTS_NOTIFICATIONS] {
        assert!(!should_hide_for_fullscreen(
            Some(state),
            0,
            start_menu,
            monitor
        ));
    }
    // A real borderless fullscreen app still hides the widget, including BUSY.
    assert!(should_hide_for_fullscreen(
        Some(QUNS_BUSY),
        0,
        monitor,
        monitor
    ));
    assert!(should_hide_for_fullscreen(
        Some(super::QUNS_PRESENTATION_MODE),
        0,
        start_menu,
        monitor
    ));
}

#[test]
fn taskbar_geometry_detects_every_edge() {
    let monitor = RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    assert!(matches!(
        taskbar_edge(
            RECT {
                left: 0,
                top: 1040,
                right: 1920,
                bottom: 1080,
            },
            monitor,
        ),
        TaskbarEdge::Bottom
    ));
    assert!(matches!(
        taskbar_edge(
            RECT {
                left: 0,
                top: 0,
                right: 48,
                bottom: 1080,
            },
            monitor,
        ),
        TaskbarEdge::Left
    ));
}

#[test]
fn hidden_auto_hide_bar_has_only_a_thin_visible_sliver() {
    let monitor = RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    let hidden = RECT {
        left: 0,
        top: 1078,
        right: 1920,
        bottom: 1120,
    };
    assert_eq!(
        visible_taskbar_thickness(hidden, monitor, TaskbarEdge::Bottom),
        2
    );
}

#[test]
fn embedded_widget_uses_only_the_gap_between_tasks_and_tray() {
    let mut placement = TaskbarPlacement {
        hwnd: HWND::default(),
        rect: RECT {
            left: 0,
            top: 1040,
            right: 1920,
            bottom: 1080,
        },
        tray_rect: Some(RECT {
            left: 1500,
            top: 1040,
            right: 1920,
            bottom: 1080,
        }),
        task_list_rect: Some(RECT {
            left: 500,
            top: 1040,
            right: 1300,
            bottom: 1080,
        }),
        widgets_enabled: None,
        monitor: MONITORINFO::default(),
        edge: TaskbarEdge::Bottom,
        auto_hide: false,
        revealed: true,
    };

    assert_eq!(
        embedded_screen_position(&placement, 100, 26, 96),
        Some((1399, 1047))
    );
    placement.task_list_rect.as_mut().expect("task list").right = 1400;
    assert_eq!(embedded_screen_position(&placement, 100, 26, 96), None);
}

#[test]
fn left_widget_uses_the_horizontal_taskbar_start_edge_without_system_widgets() {
    let mut placement = TaskbarPlacement {
        hwnd: HWND::default(),
        rect: RECT {
            left: 1920,
            top: 1040,
            right: 3840,
            bottom: 1080,
        },
        tray_rect: None,
        task_list_rect: None,
        widgets_enabled: Some(false),
        monitor: MONITORINFO::default(),
        edge: TaskbarEdge::Bottom,
        auto_hide: false,
        revealed: true,
    };

    assert_eq!(
        left_screen_position(&placement, 100, 26, 96),
        Some((1926, 1047))
    );

    placement.task_list_rect = Some(RECT {
        left: 2420,
        top: 1040,
        right: 3200,
        bottom: 1080,
    });
    assert_eq!(
        left_screen_position(&placement, 100, 26, 96),
        Some((1926, 1047))
    );
}

#[test]
fn left_widget_uses_a_safe_gap_before_tasks_when_widgets_state_is_unknown() {
    let placement = TaskbarPlacement {
        hwnd: HWND::default(),
        rect: RECT {
            left: 0,
            top: 1040,
            right: 1920,
            bottom: 1080,
        },
        tray_rect: None,
        task_list_rect: Some(RECT {
            left: 500,
            top: 1040,
            right: 1300,
            bottom: 1080,
        }),
        widgets_enabled: None,
        monitor: MONITORINFO::default(),
        edge: TaskbarEdge::Bottom,
        auto_hide: false,
        revealed: true,
    };

    assert_eq!(
        left_screen_position(&placement, 100, 26, 96),
        Some((394, 1047))
    );
}

#[test]
fn left_widget_reserves_space_before_centered_tasks_when_widgets_are_enabled() {
    let placement = TaskbarPlacement {
        hwnd: HWND::default(),
        rect: RECT {
            left: 0,
            top: 1040,
            right: 1920,
            bottom: 1080,
        },
        tray_rect: None,
        task_list_rect: Some(RECT {
            left: 500,
            top: 1040,
            right: 1300,
            bottom: 1080,
        }),
        widgets_enabled: Some(true),
        monitor: MONITORINFO::default(),
        edge: TaskbarEdge::Bottom,
        auto_hide: false,
        revealed: true,
    };

    assert_eq!(
        left_screen_position(&placement, 100, 26, 96),
        Some((394, 1047))
    );
}

#[test]
fn transparent_widget_text_follows_the_windows_theme() {
    assert_eq!(
        widget_foreground(true, WindowsWidgetStatus::Fresh),
        [32, 32, 34]
    );
    assert_eq!(
        widget_foreground(false, WindowsWidgetStatus::Fresh),
        [245, 245, 247]
    );
    assert_eq!(
        widget_foreground(true, WindowsWidgetStatus::Error),
        widget_foreground(true, WindowsWidgetStatus::Fresh)
    );
    assert_eq!(
        widget_foreground(false, WindowsWidgetStatus::Error),
        widget_foreground(false, WindowsWidgetStatus::Fresh)
    );
}

#[test]
fn text_mask_is_converted_to_premultiplied_alpha() {
    let mut pixels = vec![0; 8];
    blend_text_mask(&mut pixels, 2, 1, 0, 0, &[64, 0], 2, 1, [32, 64, 128]);
    assert_eq!(pixels, vec![32, 16, 8, 64, 0, 0, 0, 0]);
}

#[test]
fn system_title_supersampling_produces_antialiased_edges() {
    let mask = rasterize_system_title_text_mask("74%", 32, 22, 96);
    assert!(mask.iter().any(|alpha| *alpha >= 128));
    assert!(mask.iter().any(|alpha| (1..255).contains(alpha)));
    let selected_face = resolved_system_title_font_face(96);
    assert!(
        selected_face.eq_ignore_ascii_case(SYSTEM_TITLE_FONT_PRIMARY)
            || selected_face.eq_ignore_ascii_case(SYSTEM_TITLE_FONT_FALLBACK),
        "unexpected system title font: {selected_face:?}"
    );
    let source = include_str!("../windows_taskbar_widget.rs");
    assert!(!source.contains(&["Outfit", "-SemiBold"].concat()));
    assert!(!source.contains(&["OUTFIT", "_FONT_BYTES"].concat()));
}

#[test]
fn taskbar_74_percent_pixels_match_the_dpi_aware_target() {
    for dpi in [96, 120, 144, 192] {
        let (width, height) = desired_size("74%", dpi);
        assert_eq!(height, scale(BASE_SINGLE_LINE_HEIGHT, dpi));
        let pixels = render_widget_pixels("74%", width, height, dpi, [32, 32, 34]);
        assert_eq!(pixels.len(), width as usize * height as usize * 4);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
    }
}

#[test]
fn layered_widget_pixels_are_premultiplied_bgra() {
    let (width, height) = desired_size("74%", 144);
    let pixels = render_widget_pixels("74%", width, height, 144, [245, 245, 247]);
    assert!(pixels_are_premultiplied_bgra(&pixels));
}

#[test]
fn system_title_measurement_does_not_clip_rendered_glyphs() {
    for dpi in [96, 144, 192] {
        let (width, height) = measure_system_title_text("74%", dpi);
        let mask = rasterize_system_title_text_mask("74%", width as u32, height as u32, dpi);
        let mut min_x = width;
        let mut max_x = -1;
        let mut min_y = height;
        let mut max_y = -1;
        for (index, alpha) in mask.iter().copied().enumerate() {
            if alpha == 0 {
                continue;
            }
            let x = index as i32 % width;
            let y = index as i32 / width;
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        assert!(
            min_x > 0 && max_x < width - 1,
            "dpi={dpi} x={min_x}..{max_x}/{width}"
        );
        assert!(
            min_y > 0 && max_y < height - 1,
            "dpi={dpi} y={min_y}..{max_y}/{height}"
        );
    }
}

#[test]
fn two_quota_values_use_two_taskbar_lines() {
    assert_eq!(widget_text_lines("100% / 99%"), vec!["100%", "99%"]);
    assert_eq!(widget_text_lines("100%"), vec!["100%"]);
    assert!(base_height_for_text("100% / 99%") > base_height_for_text("100%"));
}

#[test]
fn taskbar_width_reserves_space_for_the_leading_icon() {
    let text_width = 28;
    let expected =
        scale(BASE_PADDING * 2 + BASE_ICON_SIZE + BASE_ICON_GAP, 144) + scale(text_width, 144);
    assert_eq!(expected, 93);
}
