use std::ffi::c_void;
use std::mem::size_of;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tauri::{AppHandle, Manager};
use windows::core::{w, BOOL, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    COLORREF, ERROR_CLASS_ALREADY_EXISTS, ERROR_SUCCESS, HINSTANCE, HWND, LPARAM, LRESULT, POINT,
    RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateRoundRectRgn, DeleteDC,
    DeleteObject, DrawTextW, EndPaint, GetMonitorInfoW, GetTextExtentPoint32W, InvalidateRect,
    MonitorFromPoint, MonitorFromWindow, SelectObject, SetBkMode, SetTextColor, SetWindowRgn,
    AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS,
    DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, HGDIOBJ, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, OUT_TT_PRECIS, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::UI::Controls::{
    TOOLTIPS_CLASSW, TTF_IDISHWND, TTF_SUBCLASS, TTM_ADDTOOLW, TTM_UPDATETIPTEXTW, TTS_ALWAYSTIP,
    TTS_NOPREFIX, TTTOOLINFOW,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{
    SHAppBarMessage, ABE_BOTTOM, ABE_LEFT, ABE_RIGHT, ABE_TOP, ABM_GETAUTOHIDEBAREX, APPBARDATA,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, EnumWindows, FindWindowExW, FindWindowW,
    GetClassNameW, GetClientRect, GetCursorPos, GetMessageW, GetWindowLongPtrW, GetWindowRect,
    LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW,
    SendMessageW, SetCursor, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    TranslateMessage, UpdateLayeredWindow, WindowFromPoint, CREATESTRUCTW, GWLP_HWNDPARENT,
    GWLP_USERDATA, HTCLIENT, HWND_TOPMOST, IDC_ARROW, IDC_HAND, MSG, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA,
    WINDOW_STYLE, WM_APP, WM_CREATE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONUP, WM_NCCREATE,
    WM_NCDESTROY, WM_NCHITTEST, WM_PAINT, WM_SETCURSOR, WM_SETTINGCHANGE, WM_THEMECHANGED,
    WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP,
};

use crate::models::WindowsTaskbarWidgetPlacement;
use crate::windows_tray_icon::codex_tools_icon_rgba;

const WINDOW_CLASS_NAME: PCWSTR = w!("CodexToolsTaskbarQuotaWidget");
const UPDATE_MESSAGE: u32 = WM_APP + 0x41;
const LAYOUT_TIMER_ID: usize = 1;
const LAYOUT_TIMER_MS: u32 = 1_000;
const BASE_SINGLE_LINE_HEIGHT: i32 = 22;
const BASE_STACKED_HEIGHT: i32 = 34;
const BASE_PADDING: i32 = 6;
const BASE_ICON_SIZE: i32 = 18;
const BASE_ICON_GAP: i32 = 4;
const BASE_EMBEDDED_GAP: i32 = 1;
const BASE_FLOATING_GAP: i32 = 4;
const BASE_EDGE_MARGIN: i32 = 12;
const BASE_LEFT_EDGE_MARGIN: i32 = 6;
const MIN_TEXT_WIDTH: i32 = 18;
const MAX_WIDTH: i32 = 260;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowsWidgetStatus {
    Fresh,
    Stale,
    Error,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowsTaskbarWidgetSnapshot {
    pub(crate) visible: bool,
    pub(crate) placement: WindowsTaskbarWidgetPlacement,
    pub(crate) text: String,
    pub(crate) tooltip: String,
    pub(crate) status: WindowsWidgetStatus,
}

struct Runtime {
    hwnd: AtomicIsize,
    snapshot: Mutex<WindowsTaskbarWidgetSnapshot>,
}

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static TASKBAR_CREATED_MESSAGE: AtomicU32 = AtomicU32::new(0);

struct WindowContext {
    app: AppHandle,
    anchor_hwnd: HWND,
    snapshot: WindowsTaskbarWidgetSnapshot,
    tooltip: Option<HWND>,
    tooltip_text: Vec<u16>,
    light_theme: bool,
    last_layout_log: String,
    taskbar_owner: Option<HWND>,
}

#[derive(Debug, Clone, Copy)]
enum TaskbarEdge {
    Left,
    Top,
    Right,
    Bottom,
}

struct TaskbarPlacement {
    hwnd: HWND,
    rect: RECT,
    tray_rect: Option<RECT>,
    task_list_rect: Option<RECT>,
    monitor: MONITORINFO,
    edge: TaskbarEdge,
    auto_hide: bool,
    revealed: bool,
}

pub(crate) fn setup(
    app: &AppHandle,
    initial_snapshot: WindowsTaskbarWidgetSnapshot,
) -> Result<(), String> {
    if RUNTIME.get().is_some() {
        return update(initial_snapshot);
    }

    RUNTIME
        .set(Runtime {
            hwnd: AtomicIsize::new(0),
            snapshot: Mutex::new(initial_snapshot),
        })
        .map_err(|_| "Windows quota widget runtime was already initialized".to_string())?;

    // Resolve HWND-backed resources on Tauri's setup thread before waiting for
    // the widget thread. Asking the WebView for its HWND from the new thread
    // while setup blocks the main thread can otherwise deadlock until timeout.
    let anchor_hwnd = app
        .get_webview_window("main")
        .and_then(|window| window.hwnd().ok())
        .unwrap_or_default();
    let anchor_hwnd_raw = anchor_hwnd.0 as isize;
    let app_handle = app.clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("codex-taskbar-quota-widget".to_string())
        .spawn(move || {
            let anchor_hwnd = HWND(anchor_hwnd_raw as *mut c_void);
            let mut ready_tx = Some(ready_tx);
            loop {
                match create_widget_window(app_handle.clone(), anchor_hwnd) {
                    Ok(hwnd) => {
                        log::info!("WINDOWS_QUOTA_WIDGET action=started");
                        if let Some(sender) = ready_tx.take() {
                            let _ = sender.send(Ok(()));
                        }
                        run_message_loop(hwnd);
                        log::info!("WINDOWS_QUOTA_WIDGET action=recreate-after-destroy");
                    }
                    Err(error) => {
                        if let Some(sender) = ready_tx.take() {
                            let _ = sender.send(Err(error));
                            return;
                        }
                        log::warn!("Windows quota widget recreation failed: {error}");
                    }
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        })
        .map_err(|error| format!("Failed to start Windows quota widget thread: {error}"))?;

    ready_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|error| format!("Timed out starting Windows quota widget: {error}"))?
}

pub(crate) fn update(snapshot: WindowsTaskbarWidgetSnapshot) -> Result<(), String> {
    let runtime = RUNTIME
        .get()
        .ok_or_else(|| "Windows quota widget is not initialized".to_string())?;
    *runtime
        .snapshot
        .lock()
        .map_err(|_| "Windows quota widget snapshot lock is poisoned".to_string())? = snapshot;

    let raw_hwnd = runtime.hwnd.load(Ordering::Acquire);
    if raw_hwnd != 0 {
        let hwnd = HWND(raw_hwnd as *mut c_void);
        unsafe {
            PostMessageW(Some(hwnd), UPDATE_MESSAGE, WPARAM(0), LPARAM(0))
                .map_err(|error| format!("Failed to notify Windows quota widget: {error}"))?;
        }
    }
    Ok(())
}

fn create_widget_window(app: AppHandle, anchor_hwnd: HWND) -> Result<HWND, String> {
    unsafe {
        let module = GetModuleHandleW(None)
            .map_err(|error| format!("Failed to resolve widget module handle: {error}"))?;
        let hinstance = HINSTANCE(module.0);
        let cursor = LoadCursorW(None, IDC_ARROW)
            .map_err(|error| format!("Failed to load widget cursor: {error}"))?;
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: Default::default(),
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: Default::default(),
            hCursor: cursor,
            hbrBackground: Default::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: WINDOW_CLASS_NAME,
            hIconSm: Default::default(),
        };

        if RegisterClassExW(&class) == 0 {
            let error = windows::Win32::Foundation::GetLastError();
            if error != ERROR_CLASS_ALREADY_EXISTS {
                return Err(format!(
                    "Failed to register Windows quota widget class: {error:?}"
                ));
            }
        }

        TASKBAR_CREATED_MESSAGE.store(
            RegisterWindowMessageW(w!("TaskbarCreated")),
            Ordering::Release,
        );

        let snapshot = RUNTIME
            .get()
            .and_then(|runtime| runtime.snapshot.lock().ok().map(|value| value.clone()))
            .ok_or_else(|| "Windows quota widget runtime is unavailable".to_string())?;
        let initial_height = base_height_for_text(&snapshot.text);
        let context = Box::new(WindowContext {
            app,
            anchor_hwnd,
            snapshot,
            tooltip: None,
            tooltip_text: Vec::new(),
            light_theme: system_uses_light_theme(),
            last_layout_log: String::new(),
            taskbar_owner: None,
        });
        let context_ptr = Box::into_raw(context);

        let hwnd = match CreateWindowExW(
            WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            WINDOW_CLASS_NAME,
            w!("Codex Tools quota"),
            WS_POPUP,
            0,
            0,
            64,
            initial_height,
            None,
            None,
            Some(hinstance),
            Some(context_ptr.cast()),
        ) {
            Ok(hwnd) => hwnd,
            Err(error) => {
                drop(Box::from_raw(context_ptr));
                return Err(format!("Failed to create Windows quota widget: {error}"));
            }
        };

        RUNTIME
            .get()
            .expect("runtime initialized before widget window")
            .hwnd
            .store(hwnd.0 as isize, Ordering::Release);
        SetTimer(Some(hwnd), LAYOUT_TIMER_ID, LAYOUT_TIMER_MS, None);
        PostMessageW(Some(hwnd), UPDATE_MESSAGE, WPARAM(0), LPARAM(0))
            .map_err(|error| format!("Failed to initialize Windows quota widget: {error}"))?;
        Ok(hwnd)
    }
}

fn run_message_loop(_hwnd: HWND) {
    unsafe {
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let taskbar_created = TASKBAR_CREATED_MESSAGE.load(Ordering::Acquire);
    if taskbar_created != 0 && message == taskbar_created {
        log::info!("WINDOWS_QUOTA_WIDGET action=taskbar-created");
        apply_snapshot_and_layout(hwnd);
        return LRESULT(0);
    }

    match message {
        WM_NCCREATE => {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
            LRESULT(1)
        }
        WM_CREATE => {
            if let Some(context) = context_mut(hwnd) {
                context.tooltip = create_tooltip(hwnd, context);
            }
            apply_snapshot_and_layout(hwnd);
            LRESULT(0)
        }
        UPDATE_MESSAGE => {
            apply_snapshot_and_layout(hwnd);
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == LAYOUT_TIMER_ID => {
            let light_theme = system_uses_light_theme();
            if let Some(context) = context_mut(hwnd) {
                if context.light_theme != light_theme {
                    context.light_theme = light_theme;
                    let _ = InvalidateRect(Some(hwnd), None, true);
                }
            }
            position_widget(hwnd);
            LRESULT(0)
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED | WM_DISPLAYCHANGE | WM_DPICHANGED => {
            if let Some(context) = context_mut(hwnd) {
                context.light_theme = system_uses_light_theme();
            }
            apply_snapshot_and_layout(hwnd);
            LRESULT(0)
        }
        WM_PAINT => {
            paint_widget(hwnd);
            LRESULT(0)
        }
        WM_SETCURSOR => {
            if let Ok(cursor) = LoadCursorW(None, IDC_HAND) {
                SetCursor(Some(cursor));
                return LRESULT(1);
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        WM_NCHITTEST => LRESULT(HTCLIENT as isize),
        WM_LBUTTONUP => {
            if let Some(context) = context_mut(hwnd) {
                log::info!("WINDOWS_QUOTA_WIDGET action=click-restore");
                crate::restore_main_window(&context.app);
            }
            LRESULT(0)
        }
        WM_NCDESTROY => {
            let raw_context = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowContext;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            if !raw_context.is_null() {
                drop(Box::from_raw(raw_context));
            }
            if let Some(runtime) = RUNTIME.get() {
                runtime.hwnd.store(0, Ordering::Release);
            }
            PostQuitMessage(0);
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn context_mut(hwnd: HWND) -> Option<&'static mut WindowContext> {
    let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowContext;
    raw.as_mut()
}

unsafe fn apply_snapshot_and_layout(hwnd: HWND) {
    let Some(snapshot) = RUNTIME
        .get()
        .and_then(|runtime| runtime.snapshot.lock().ok().map(|value| value.clone()))
    else {
        return;
    };

    if let Some(context) = context_mut(hwnd) {
        if context.snapshot != snapshot {
            log::info!(
                "WINDOWS_QUOTA_WIDGET_SNAPSHOT visible={} status={:?} text={:?}",
                snapshot.visible,
                snapshot.status,
                snapshot.text
            );
        }
        context.snapshot = snapshot;
        update_tooltip(hwnd, context);
    }
    let _ = InvalidateRect(Some(hwnd), None, true);
    position_widget(hwnd);
}

unsafe fn create_tooltip(hwnd: HWND, context: &mut WindowContext) -> Option<HWND> {
    let module = GetModuleHandleW(None).ok()?;
    let tooltip = CreateWindowExW(
        WS_EX_TOPMOST,
        TOOLTIPS_CLASSW,
        PCWSTR::null(),
        WINDOW_STYLE(WS_POPUP.0 | TTS_ALWAYSTIP | TTS_NOPREFIX),
        0,
        0,
        0,
        0,
        Some(hwnd),
        None,
        Some(HINSTANCE(module.0)),
        None,
    )
    .ok()?;
    SetWindowPos(
        tooltip,
        Some(HWND_TOPMOST),
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    )
    .ok()?;

    context.tooltip_text = to_wide(&context.snapshot.tooltip);
    let tool = tooltip_info(hwnd, &mut context.tooltip_text);
    SendMessageW(
        tooltip,
        TTM_ADDTOOLW,
        None,
        Some(LPARAM((&tool as *const TTTOOLINFOW) as isize)),
    );
    Some(tooltip)
}

unsafe fn update_tooltip(hwnd: HWND, context: &mut WindowContext) {
    let Some(tooltip) = context.tooltip else {
        return;
    };
    context.tooltip_text = to_wide(&context.snapshot.tooltip);
    let tool = tooltip_info(hwnd, &mut context.tooltip_text);
    SendMessageW(
        tooltip,
        TTM_UPDATETIPTEXTW,
        None,
        Some(LPARAM((&tool as *const TTTOOLINFOW) as isize)),
    );
}

fn tooltip_info(hwnd: HWND, text: &mut [u16]) -> TTTOOLINFOW {
    TTTOOLINFOW {
        cbSize: size_of::<TTTOOLINFOW>() as u32,
        uFlags: TTF_IDISHWND | TTF_SUBCLASS,
        hwnd,
        uId: hwnd.0 as usize,
        lpszText: PWSTR(text.as_mut_ptr()),
        ..Default::default()
    }
}

unsafe fn paint_widget(hwnd: HWND) {
    let Some(context) = context_mut(hwnd) else {
        return;
    };
    let mut paint = PAINTSTRUCT::default();
    let _ = BeginPaint(hwnd, &mut paint);
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    if client.right > 0 && client.bottom > 0 {
        render_layered_text(hwnd, context, client.right, client.bottom);
    }
    let _ = EndPaint(hwnd, &paint);
}

unsafe fn render_layered_text(hwnd: HWND, context: &WindowContext, width: i32, height: i32) {
    let memory_dc = CreateCompatibleDC(None);
    if memory_dc.is_invalid() {
        return;
    }

    let mut bits: *mut c_void = std::ptr::null_mut();
    let bitmap_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let Ok(bitmap) = CreateDIBSection(
        Some(memory_dc),
        &bitmap_info,
        DIB_RGB_COLORS,
        &mut bits,
        None,
        0,
    ) else {
        let _ = DeleteDC(memory_dc);
        return;
    };
    if bits.is_null() {
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        return;
    }

    let previous_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
    let byte_len = (width as usize) * (height as usize) * 4;
    let background = taskbar_background(context.light_theme);
    let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), byte_len);
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[background[2], background[1], background[0], 255]);
    }
    let dpi = GetDpiForWindow(hwnd).max(96);
    let font = CreateFontW(
        -scale(11, dpi),
        0,
        0,
        0,
        500,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_TT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY,
        u32::from(DEFAULT_PITCH.0 | FF_DONTCARE.0),
        w!("Segoe UI"),
    );
    let previous_font = SelectObject(memory_dc, HGDIOBJ(font.0));
    SetBkMode(memory_dc, TRANSPARENT);
    let foreground = widget_foreground(context.light_theme, context.snapshot.status);
    SetTextColor(memory_dc, color_ref(foreground));
    let padding = scale(BASE_PADDING, dpi);
    let icon_size = scale(BASE_ICON_SIZE, dpi);
    let icon_gap = scale(BASE_ICON_GAP, dpi);
    draw_taskbar_icon(pixels, width, height, padding, icon_size, background);
    let text_left = padding + icon_size + icon_gap;
    let lines = widget_text_lines(&context.snapshot.text);
    for (index, line) in lines.iter().enumerate() {
        let mut text = line.encode_utf16().collect::<Vec<_>>();
        let mut text_rect = if lines.len() == 1 {
            RECT {
                left: text_left,
                top: 0,
                right: width - padding,
                bottom: height,
            }
        } else {
            let midpoint = height / 2;
            RECT {
                left: text_left,
                top: if index == 0 { 0 } else { midpoint },
                right: width - padding,
                bottom: if index == 0 { midpoint } else { height },
            }
        };
        DrawTextW(
            memory_dc,
            &mut text,
            &mut text_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
    }

    for pixel in pixels.chunks_exact_mut(4) {
        if pixel[0] == background[2] && pixel[1] == background[1] && pixel[2] == background[0] {
            pixel.fill(0);
        } else {
            // Keep ClearType's per-channel coverage intact. The edge pixels
            // are already composited against the matching taskbar color.
            pixel[3] = 255;
        }
    }

    let size = SIZE {
        cx: width,
        cy: height,
    };
    let source = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let _ = UpdateLayeredWindow(
        hwnd,
        None,
        None,
        Some(&size),
        Some(memory_dc),
        Some(&source),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );

    SelectObject(memory_dc, previous_font);
    SelectObject(memory_dc, previous_bitmap);
    let _ = DeleteObject(HGDIOBJ(font.0));
    let _ = DeleteObject(HGDIOBJ(bitmap.0));
    let _ = DeleteDC(memory_dc);
}

fn widget_foreground(light_theme: bool, status: WindowsWidgetStatus) -> [u8; 3] {
    match (light_theme, status) {
        (true, WindowsWidgetStatus::Fresh) => [32, 32, 34],
        (false, WindowsWidgetStatus::Fresh) => [245, 245, 247],
        (true, WindowsWidgetStatus::Stale) => [104, 70, 0],
        (false, WindowsWidgetStatus::Stale) => [255, 225, 143],
        (true, WindowsWidgetStatus::Error) => [32, 32, 34],
        (false, WindowsWidgetStatus::Error) => [245, 245, 247],
        (true, WindowsWidgetStatus::Unavailable) => [83, 91, 99],
        (false, WindowsWidgetStatus::Unavailable) => [207, 211, 216],
    }
}

fn draw_taskbar_icon(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    left: i32,
    icon_size: i32,
    background: [u8; 3],
) {
    if width <= 0 || height <= 0 || icon_size <= 0 {
        return;
    }
    let icon = codex_tools_icon_rgba(icon_size as u32);
    let top = ((height - icon_size) / 2).max(0);
    for icon_y in 0..icon_size {
        let target_y = top + icon_y;
        if target_y < 0 || target_y >= height {
            continue;
        }
        for icon_x in 0..icon_size {
            let target_x = left + icon_x;
            if target_x < 0 || target_x >= width {
                continue;
            }
            let source_index = ((icon_y * icon_size + icon_x) * 4) as usize;
            let target_index = ((target_y * width + target_x) * 4) as usize;
            let alpha = icon[source_index + 3] as u16;
            if alpha == 0 {
                continue;
            }
            for channel in 0..3 {
                let source = icon[source_index + channel] as u16;
                let backdrop = background[channel] as u16;
                let blended = (source * alpha + backdrop * (255 - alpha) + 127) / 255;
                // The layered DIB is BGRA while the icon and theme colors are RGBA/RGB.
                pixels[target_index + (2 - channel)] = blended as u8;
            }
        }
    }
}

fn taskbar_background(light_theme: bool) -> [u8; 3] {
    if light_theme {
        [243, 243, 243]
    } else {
        [28, 28, 28]
    }
}

fn color_ref(color: [u8; 3]) -> COLORREF {
    COLORREF(color[0] as u32 | ((color[1] as u32) << 8) | ((color[2] as u32) << 16))
}

fn widget_text_lines(text: &str) -> Vec<&str> {
    match text.split_once(" / ") {
        Some((primary, secondary)) => vec![primary.trim(), secondary.trim()],
        None => vec![text.trim()],
    }
}

fn base_height_for_text(text: &str) -> i32 {
    if widget_text_lines(text).len() > 1 {
        BASE_STACKED_HEIGHT
    } else {
        BASE_SINGLE_LINE_HEIGHT
    }
}

unsafe fn position_widget(hwnd: HWND) {
    let Some(context) = context_mut(hwnd) else {
        return;
    };
    if !context.snapshot.visible
        || context.snapshot.placement == WindowsTaskbarWidgetPlacement::Hidden
    {
        clear_taskbar_owner(hwnd, context);
        log_layout_change(context, "visible=false reason=setting-hidden".to_string());
        let _ = ShowWindow(hwnd, SW_HIDE);
        return;
    }

    let Some(taskbar) = locate_taskbar(context.anchor_hwnd) else {
        log_layout_change(
            context,
            "visible=false reason=taskbar-unavailable".to_string(),
        );
        let _ = ShowWindow(hwnd, SW_HIDE);
        return;
    };
    if taskbar.auto_hide && !taskbar.revealed {
        log_layout_change(
            context,
            format!(
                "visible=false reason=taskbar-auto-hidden edge={:?} dpi={}",
                taskbar.edge,
                GetDpiForWindow(hwnd).max(96)
            ),
        );
        let _ = ShowWindow(hwnd, SW_HIDE);
        return;
    }

    let dpi = GetDpiForWindow(hwnd).max(96);
    let (width, height) = desired_size(hwnd, &context.snapshot.text, dpi);
    if matches!(
        context.snapshot.placement,
        WindowsTaskbarWidgetPlacement::Embedded | WindowsTaskbarWidgetPlacement::Left
    ) {
        let (placement_name, screen_position) = match context.snapshot.placement {
            WindowsTaskbarWidgetPlacement::Embedded => (
                "embedded",
                embedded_screen_position(&taskbar, width, height, dpi),
            ),
            WindowsTaskbarWidgetPlacement::Left => {
                ("left", left_screen_position(&taskbar, width, height, dpi))
            }
            _ => unreachable!("filtered to taskbar-owned placements"),
        };
        if let Some((screen_x, screen_y)) = screen_position {
            if own_by_taskbar(hwnd, context, taskbar.hwnd) {
                apply_widget_region(hwnd, width, height, dpi);
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    screen_x,
                    screen_y,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                let center_hit = WindowFromPoint(POINT {
                    x: screen_x + width / 2,
                    y: screen_y + height / 2,
                });
                log_layout_change(
                    context,
                    format!(
                        "visible=true placement={} surface=taskbar-owned-overlay background=per-pixel-transparent edge={:?} dpi={} owner={:?} taskbar=({},{},{},{}) bounds=({},{},{},{}) center_hit={:?} owns_center={}",
                        placement_name,
                        taskbar.edge,
                        dpi,
                        taskbar.hwnd,
                        taskbar.rect.left,
                        taskbar.rect.top,
                        taskbar.rect.right,
                        taskbar.rect.bottom,
                        screen_x,
                        screen_y,
                        width,
                        height,
                        center_hit,
                        center_hit == hwnd,
                    ),
                );
                return;
            }
        }
        log_layout_change(
            context,
            format!(
                "placement={} action=fallback-floating reason=no-safe-taskbar-position",
                placement_name
            ),
        );
    }

    clear_taskbar_owner(hwnd, context);
    position_floating_widget(hwnd, context, &taskbar, width, height, dpi);
}

unsafe fn position_floating_widget(
    hwnd: HWND,
    context: &mut WindowContext,
    taskbar: &TaskbarPlacement,
    width: i32,
    height: i32,
    dpi: u32,
) {
    let gap = scale(BASE_FLOATING_GAP, dpi);
    let edge_margin = scale(BASE_EDGE_MARGIN, dpi);
    let monitor = taskbar.monitor.rcMonitor;
    let (x, y) = match taskbar.edge {
        TaskbarEdge::Bottom => (
            monitor.right - width - edge_margin,
            taskbar.rect.top - height - gap,
        ),
        TaskbarEdge::Top => (
            monitor.right - width - edge_margin,
            taskbar.rect.bottom + gap,
        ),
        TaskbarEdge::Left => (
            taskbar.rect.right + gap,
            monitor.bottom - height - edge_margin,
        ),
        TaskbarEdge::Right => (
            taskbar.rect.left - width - gap,
            monitor.bottom - height - edge_margin,
        ),
    };

    apply_widget_region(hwnd, width, height, dpi);
    let _ = SetWindowPos(
        hwnd,
        Some(HWND_TOPMOST),
        x,
        y,
        width,
        height,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    log_layout_change(
        context,
        format!(
            "visible=true placement=floating background=per-pixel-transparent edge={:?} auto_hide={} revealed={} dpi={} monitor=({},{},{},{}) taskbar=({},{},{},{}) bounds=({},{},{},{})",
            taskbar.edge,
            taskbar.auto_hide,
            taskbar.revealed,
            dpi,
            monitor.left,
            monitor.top,
            monitor.right,
            monitor.bottom,
            taskbar.rect.left,
            taskbar.rect.top,
            taskbar.rect.right,
            taskbar.rect.bottom,
            x,
            y,
            width,
            height
        ),
    );
}

unsafe fn apply_widget_region(hwnd: HWND, width: i32, height: i32, dpi: u32) {
    let radius = scale(9, dpi);
    let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, radius, radius);
    if SetWindowRgn(hwnd, Some(region), true) == 0 {
        let _ = DeleteObject(HGDIOBJ(region.0));
    }
}

fn embedded_screen_position(
    taskbar: &TaskbarPlacement,
    width: i32,
    height: i32,
    dpi: u32,
) -> Option<(i32, i32)> {
    if !matches!(taskbar.edge, TaskbarEdge::Bottom | TaskbarEdge::Top) {
        return None;
    }
    let tray = taskbar.tray_rect?;
    let gap = scale(BASE_EMBEDDED_GAP, dpi);
    let occupied_right = taskbar
        .task_list_rect
        .map(|rect| rect.right)
        .unwrap_or(taskbar.rect.left + gap);
    let x = tray.left - gap - width;
    if x < occupied_right + gap || x < taskbar.rect.left + gap {
        return None;
    }
    let taskbar_height = taskbar.rect.bottom - taskbar.rect.top;
    if height + gap * 2 > taskbar_height {
        return None;
    }
    let y = taskbar.rect.top + (taskbar_height - height) / 2;
    Some((x, y))
}

fn left_screen_position(
    taskbar: &TaskbarPlacement,
    width: i32,
    height: i32,
    dpi: u32,
) -> Option<(i32, i32)> {
    if !matches!(taskbar.edge, TaskbarEdge::Bottom | TaskbarEdge::Top) {
        return None;
    }
    let margin = scale(BASE_LEFT_EDGE_MARGIN, dpi);
    let taskbar_width = taskbar.rect.right - taskbar.rect.left;
    let taskbar_height = taskbar.rect.bottom - taskbar.rect.top;
    if width + margin * 2 > taskbar_width || height + margin * 2 > taskbar_height {
        return None;
    }
    Some((
        taskbar.rect.left + margin,
        taskbar.rect.top + (taskbar_height - height) / 2,
    ))
}

unsafe fn own_by_taskbar(hwnd: HWND, context: &mut WindowContext, owner: HWND) -> bool {
    let owner_raw = owner.0 as isize;
    if context.taskbar_owner == Some(owner) && GetWindowLongPtrW(hwnd, GWLP_HWNDPARENT) == owner_raw
    {
        return true;
    }

    SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, owner_raw);
    let owned = GetWindowLongPtrW(hwnd, GWLP_HWNDPARENT) == owner_raw;
    if owned {
        context.taskbar_owner = Some(owner);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
        return true;
    }

    context.taskbar_owner = None;
    false
}

unsafe fn clear_taskbar_owner(hwnd: HWND, context: &mut WindowContext) {
    if context.taskbar_owner.is_none() && GetWindowLongPtrW(hwnd, GWLP_HWNDPARENT) == 0 {
        return;
    }
    SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, 0);
    context.taskbar_owner = None;
    let _ = SetWindowPos(
        hwnd,
        Some(HWND_TOPMOST),
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
    );
}

fn log_layout_change(context: &mut WindowContext, detail: String) {
    if context.last_layout_log != detail {
        log::info!("WINDOWS_QUOTA_WIDGET_LAYOUT {detail}");
        context.last_layout_log = detail;
    }
}

unsafe fn desired_size(hwnd: HWND, text: &str, dpi: u32) -> (i32, i32) {
    let lines = widget_text_lines(text);
    let height = scale(base_height_for_text(text), dpi);
    let padding = scale(BASE_PADDING, dpi);
    let icon_size = scale(BASE_ICON_SIZE, dpi);
    let icon_gap = scale(BASE_ICON_GAP, dpi);
    let longest_line_length = lines
        .iter()
        .map(|line| line.encode_utf16().count() as i32)
        .max()
        .unwrap_or_default();
    let fallback_text_width = scale(MIN_TEXT_WIDTH.max(longest_line_length * 7), dpi);

    let mut text_width = fallback_text_width;
    let hdc = windows::Win32::Graphics::Gdi::GetDC(Some(hwnd));
    if !hdc.is_invalid() {
        let font = CreateFontW(
            -scale(11, dpi),
            0,
            0,
            0,
            500,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_TT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            u32::from(DEFAULT_PITCH.0 | FF_DONTCARE.0),
            w!("Segoe UI"),
        );
        let previous = SelectObject(hdc, HGDIOBJ(font.0));
        for line in lines {
            let text_wide = line.encode_utf16().collect::<Vec<_>>();
            let mut size = SIZE::default();
            if GetTextExtentPoint32W(hdc, &text_wide, &mut size).as_bool() {
                text_width = text_width.max(size.cx.max(scale(MIN_TEXT_WIDTH, dpi)));
            }
        }
        SelectObject(hdc, previous);
        let _ = DeleteObject(HGDIOBJ(font.0));
        windows::Win32::Graphics::Gdi::ReleaseDC(Some(hwnd), hdc);
    }

    (
        (padding * 2 + icon_size + icon_gap + text_width).min(scale(MAX_WIDTH, dpi)),
        height,
    )
}

unsafe fn locate_taskbar(anchor_hwnd: HWND) -> Option<TaskbarPlacement> {
    let target_monitor = if anchor_hwnd.0.is_null() {
        let mut cursor = POINT::default();
        GetCursorPos(&mut cursor)
            .ok()
            .map(|_| MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST))
    } else {
        Some(MonitorFromWindow(anchor_hwnd, MONITOR_DEFAULTTONEAREST))
    }?;

    let mut search = TaskbarSearch {
        target_monitor,
        found: None,
    };
    let _ = EnumWindows(
        Some(enum_taskbars),
        LPARAM((&mut search as *mut TaskbarSearch) as isize),
    );
    let taskbar = search
        .found
        .or_else(|| FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()).ok())?;
    let mut rect = RECT::default();
    GetWindowRect(taskbar, &mut rect).ok()?;
    let monitor_handle = MonitorFromWindow(taskbar, MONITOR_DEFAULTTONEAREST);
    let mut monitor = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !GetMonitorInfoW(monitor_handle, &mut monitor).as_bool() {
        return None;
    }

    let edge = taskbar_edge(rect, monitor.rcMonitor);
    let auto_hide = taskbar_is_auto_hide(edge, monitor.rcMonitor);
    let revealed = visible_taskbar_thickness(rect, monitor.rcMonitor, edge) > 2;
    let tray_rect = child_window_rect(taskbar, w!("TrayNotifyWnd"));
    let task_list_rect = child_window_rect(taskbar, w!("ReBarWindow32"));
    Some(TaskbarPlacement {
        hwnd: taskbar,
        rect,
        tray_rect,
        task_list_rect,
        monitor,
        edge,
        auto_hide,
        revealed,
    })
}

unsafe fn child_window_rect(parent: HWND, class_name: PCWSTR) -> Option<RECT> {
    let child = FindWindowExW(Some(parent), None, class_name, PCWSTR::null()).ok()?;
    let mut rect = RECT::default();
    GetWindowRect(child, &mut rect).ok()?;
    Some(rect)
}

struct TaskbarSearch {
    target_monitor: windows::Win32::Graphics::Gdi::HMONITOR,
    found: Option<HWND>,
}

unsafe extern "system" fn enum_taskbars(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let search = &mut *(lparam.0 as *mut TaskbarSearch);
    let mut class_name = [0_u16; 64];
    let length = GetClassNameW(hwnd, &mut class_name);
    if length <= 0 {
        return true.into();
    }
    let class_name = String::from_utf16_lossy(&class_name[..length as usize]);
    if class_name != "Shell_TrayWnd" && class_name != "Shell_SecondaryTrayWnd" {
        return true.into();
    }
    if MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) == search.target_monitor {
        search.found = Some(hwnd);
        return false.into();
    }
    true.into()
}

fn taskbar_edge(rect: RECT, monitor: RECT) -> TaskbarEdge {
    let horizontal = (rect.right - rect.left).abs() >= (rect.bottom - rect.top).abs();
    let distances = if horizontal {
        [
            ((rect.top - monitor.top).abs(), TaskbarEdge::Top),
            ((monitor.bottom - rect.bottom).abs(), TaskbarEdge::Bottom),
        ]
    } else {
        [
            ((rect.left - monitor.left).abs(), TaskbarEdge::Left),
            ((monitor.right - rect.right).abs(), TaskbarEdge::Right),
        ]
    };
    distances
        .into_iter()
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, edge)| edge)
        .unwrap_or(TaskbarEdge::Bottom)
}

fn visible_taskbar_thickness(rect: RECT, monitor: RECT, edge: TaskbarEdge) -> i32 {
    match edge {
        TaskbarEdge::Left => (rect.right.min(monitor.right) - monitor.left).max(0),
        TaskbarEdge::Top => (rect.bottom.min(monitor.bottom) - monitor.top).max(0),
        TaskbarEdge::Right => (monitor.right - rect.left.max(monitor.left)).max(0),
        TaskbarEdge::Bottom => (monitor.bottom - rect.top.max(monitor.top)).max(0),
    }
}

unsafe fn taskbar_is_auto_hide(edge: TaskbarEdge, monitor: RECT) -> bool {
    let mut data = APPBARDATA {
        cbSize: size_of::<APPBARDATA>() as u32,
        uEdge: match edge {
            TaskbarEdge::Left => ABE_LEFT,
            TaskbarEdge::Top => ABE_TOP,
            TaskbarEdge::Right => ABE_RIGHT,
            TaskbarEdge::Bottom => ABE_BOTTOM,
        },
        rc: monitor,
        ..Default::default()
    };
    SHAppBarMessage(ABM_GETAUTOHIDEBAREX, &mut data) != 0
}

pub(crate) fn system_uses_light_theme() -> bool {
    unsafe {
        let mut value = 1_u32;
        let mut size = size_of::<u32>() as u32;
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        );
        status != ERROR_SUCCESS || value != 0
    }
}

fn scale(value: i32, dpi: u32) -> i32 {
    ((value as i64 * dpi as i64 + 48) / 96) as i32
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        base_height_for_text, embedded_screen_position, left_screen_position, scale,
        taskbar_background, taskbar_edge, visible_taskbar_thickness, widget_foreground,
        widget_text_lines, TaskbarEdge, TaskbarPlacement, WindowsWidgetStatus, BASE_ICON_GAP,
        BASE_ICON_SIZE, BASE_PADDING,
    };
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::MONITORINFO;

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
    fn left_widget_uses_the_horizontal_taskbar_start_edge() {
        let placement = TaskbarPlacement {
            hwnd: HWND::default(),
            rect: RECT {
                left: 1920,
                top: 1040,
                right: 3840,
                bottom: 1080,
            },
            tray_rect: None,
            task_list_rect: None,
            monitor: MONITORINFO::default(),
            edge: TaskbarEdge::Bottom,
            auto_hide: false,
            revealed: true,
        };

        assert_eq!(
            left_screen_position(&placement, 100, 26, 96),
            Some((1926, 1047))
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
        assert_eq!(taskbar_background(true), [243, 243, 243]);
        assert_eq!(taskbar_background(false), [28, 28, 28]);
    }

    #[test]
    fn two_quota_values_use_two_compact_lines() {
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
}
