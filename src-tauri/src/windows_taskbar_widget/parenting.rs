use super::*;

pub(super) fn rects_overlap(a: RECT, b: RECT) -> bool {
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

pub(super) unsafe fn is_taskbar_child(hwnd: HWND) -> bool {
    GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_CHILD.0 != 0
}

// TrafficMonitor's Win11TaskbarDlg attaches to Shell_TrayWnd. A native child
// moves through the desktop Z order with its parent, so activating the taskbar
// cannot insert the taskbar between the quota and its own background.
pub(super) unsafe fn set_widget_parent(hwnd: HWND, parent: Option<HWND>) -> bool {
    let old_style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
    let current_parent = if is_taskbar_child(hwnd) {
        GetParent(hwnd).ok()
    } else {
        None
    };
    if current_parent == parent {
        return true;
    }
    let _ = ShowWindow(hwnd, SW_HIDE);
    if parent.is_some() {
        SetWindowLongPtrW(
            hwnd,
            GWL_STYLE,
            ((old_style & !WS_POPUP.0) | WS_CHILD.0 | WS_CLIPSIBLINGS.0) as isize,
        );
    }
    // A successful attach from the desktop also returns NULL (previous parent).
    // Verify the relationship instead of treating the return handle as a bool.
    let result = SetParent(hwnd, parent);
    if parent.is_none() {
        SetWindowLongPtrW(
            hwnd,
            GWL_STYLE,
            ((old_style & !(WS_CHILD.0 | WS_CLIPSIBLINGS.0)) | WS_POPUP.0) as isize,
        );
    }
    if GetParent(hwnd).ok() != parent {
        SetWindowLongPtrW(hwnd, GWL_STYLE, old_style as isize);
        log::warn!("WINDOWS_QUOTA_WIDGET action=parent-change-failed error={result:?}");
        return false;
    }
    true
}

pub(super) unsafe fn covering_window_above(hwnd: HWND) -> bool {
    let mut widget_rect = RECT::default();
    if GetWindowRect(hwnd, &mut widget_rect).is_err() {
        return false;
    }
    let mut above = GetWindow(hwnd, GW_HWNDPREV).ok();
    while let Some(window) = above {
        // Keep our tooltip above its owner; only repair actual visible occlusion.
        if IsWindowVisible(window).as_bool()
            && !IsIconic(window).as_bool()
            && GetWindow(window, GW_OWNER).ok() != Some(hwnd)
        {
            let mut rect = RECT::default();
            if GetWindowRect(window, &mut rect).is_ok() && rects_overlap(widget_rect, rect) {
                return true;
            }
        }
        above = GetWindow(window, GW_HWNDPREV).ok();
    }
    false
}

pub(super) unsafe fn restore_widget_z_order(hwnd: HWND) -> bool {
    let topmost = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST.0 != 0;
    if topmost && !covering_window_above(hwnd) {
        return false;
    }
    // Never activate the widget, resize it, or redraw the quota just to fix Z order.
    SetWindowPos(
        hwnd,
        Some(HWND_TOPMOST),
        0,
        0,
        0,
        0,
        SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
    )
    .is_ok()
}

pub(super) unsafe fn maintain_widget_visibility(hwnd: HWND) {
    let Some(context) = context_mut(hwnd) else {
        return;
    };
    if !context.snapshot.visible
        || context.snapshot.placement == WindowsTaskbarWidgetPlacement::Hidden
    {
        return;
    }
    let Some(taskbar) = context.taskbar else {
        return;
    };
    let mut rect = RECT::default();
    if GetWindowRect(taskbar.hwnd, &mut rect).is_err() {
        return;
    }
    let hidden_taskbar = taskbar.auto_hide
        && visible_taskbar_thickness(rect, taskbar.monitor.rcMonitor, taskbar.edge) <= 2;
    let visible = IsWindowVisible(hwnd).as_bool();
    if hidden_taskbar || foreground_window_covers_monitor(taskbar.monitor.rcMonitor) {
        if visible {
            context.surface_needs_refresh = true;
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    } else if !visible {
        if position_widget(hwnd) {
            refresh_widget_surface(hwnd);
        }
    } else if !is_taskbar_child(hwnd) {
        restore_widget_z_order(hwnd);
    }
}
