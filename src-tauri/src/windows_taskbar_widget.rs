use std::ffi::c_void;
use std::mem::size_of;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use windows::core::{w, BOOL, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    COLORREF, ERROR_CLASS_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HINSTANCE, HWND,
    LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateRoundRectRgn, DeleteDC,
    DeleteObject, DrawTextW, EndPaint, GetMonitorInfoW, GetTextExtentPoint32W, GetTextFaceW,
    InvalidateRect, MonitorFromPoint, MonitorFromWindow, ScreenToClient, SelectObject, SetBkMode,
    SetTextColor, SetWindowRgn, AC_SRC_ALPHA, AC_SRC_OVER, ANTIALIASED_QUALITY, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH,
    DIB_RGB_COLORS, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, HDC, HFONT,
    HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTONEAREST, OUT_TT_PRECIS, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, TreeScope_Descendants};
use windows::Win32::UI::Controls::{
    TOOLTIPS_CLASSW, TTF_IDISHWND, TTF_SUBCLASS, TTM_ADDTOOLW, TTM_UPDATETIPTEXTW, TTS_ALWAYSTIP,
    TTS_NOPREFIX, TTTOOLINFOW,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{
    SHAppBarMessage, SHQueryUserNotificationState, ABE_BOTTOM, ABE_LEFT, ABE_RIGHT, ABE_TOP,
    ABM_GETAUTOHIDEBAREX, APPBARDATA, QUNS_BUSY, QUNS_PRESENTATION_MODE,
    QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, EnumWindows, FindWindowExW, FindWindowW,
    GetClassNameW, GetClientRect, GetCursorPos, GetForegroundWindow, GetMessageW,
    GetWindowLongPtrW, GetWindowRect, IsIconic, LoadCursorW, PostMessageW, PostQuitMessage,
    RegisterClassExW, RegisterWindowMessageW, SendMessageW, SetCursor, SetParent, SetTimer,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow,
    WindowFromPoint, CREATESTRUCTW, GWLP_HWNDPARENT, GWLP_USERDATA, GWL_EXSTYLE, GWL_STYLE,
    HTCLIENT, HWND_TOP, HWND_TOPMOST, IDC_ARROW, IDC_HAND, MSG, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WINDOW_STYLE,
    WM_APP, WM_CREATE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_LBUTTONUP, WM_NCCREATE, WM_NCDESTROY,
    WM_NCHITTEST, WM_PAINT, WM_SETCURSOR, WM_SETTINGCHANGE, WM_THEMECHANGED, WM_TIMER, WNDCLASSEXW,
    WS_CHILD, WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_MAXIMIZE, WS_POPUP,
};

use crate::models::WindowsTaskbarWidgetPlacement;
use crate::windows_tray_icon::codex_tools_icon_rgba;

const WINDOW_CLASS_NAME: PCWSTR = w!("CodexToolsTaskbarQuotaWidget");
const UPDATE_MESSAGE: u32 = WM_APP + 0x41;
const LAYOUT_TIMER_ID: usize = 1;
const LAYOUT_TIMER_MS: u32 = 1_000;
const WIDGETS_SCAN_INTERVAL: Duration = Duration::from_secs(30);
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
const WIDGET_FONT_SIZE_DIP: f32 = 13.0;
const WIDGET_FONT_WEIGHT: i32 = 400;
const TEXT_SUPERSAMPLE: u32 = 4;
const SYSTEM_TITLE_FONT_PRIMARY: &str = "Segoe UI Variable Text";
const SYSTEM_TITLE_FONT_FALLBACK: &str = "Segoe UI";

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
    taskbar_parent: Option<HWND>,
    automation: Option<IUIAutomation>,
    cached_widgets_button_rect: Option<RECT>,
    cached_widgets_enabled: Option<bool>,
    last_widgets_scan: Option<Instant>,
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
    widgets_enabled: Option<bool>,
    widgets_button_rect: Option<RECT>,
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
            let com_result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if com_result.is_err() {
                log::warn!(
                    "Windows quota widget UI Automation initialization failed: {com_result:?}"
                );
            }
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
        let automation = match CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) {
            Ok(automation) => Some(automation),
            Err(error) => {
                log::warn!("Windows quota widget UI Automation unavailable: {error}");
                None
            }
        };
        let initial_height = base_height_for_text(&snapshot.text);
        let context = Box::new(WindowContext {
            app,
            anchor_hwnd,
            snapshot,
            tooltip: None,
            tooltip_text: Vec::new(),
            light_theme: system_uses_light_theme(),
            last_layout_log: String::new(),
            taskbar_parent: None,
            automation,
            cached_widgets_button_rect: None,
            cached_widgets_enabled: None,
            last_widgets_scan: None,
        });
        let context_ptr = Box::into_raw(context);

        let hwnd = match CreateWindowExW(
            WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
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

fn run_message_loop(hwnd: HWND) {
    unsafe {
        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&mut message, None, 0, 0).0;
            if result == 0 {
                break;
            }
            if result == -1 {
                log::warn!("Windows quota widget message loop failed");
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
                break;
            }
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
        if let Some(context) = context_mut(hwnd) {
            context.cached_widgets_button_rect = None;
            context.cached_widgets_enabled = None;
            context.last_widgets_scan = None;
        }
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
        WM_SETTINGCHANGE | WM_DISPLAYCHANGE | WM_DPICHANGED => {
            if let Some(context) = context_mut(hwnd) {
                context.light_theme = system_uses_light_theme();
                context.cached_widgets_button_rect = None;
                context.cached_widgets_enabled = None;
                context.last_widgets_scan = None;
            }
            apply_snapshot_and_layout(hwnd);
            LRESULT(0)
        }
        WM_THEMECHANGED => {
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
    let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), byte_len);
    let dpi = GetDpiForWindow(hwnd).max(96);
    let foreground = widget_foreground(context.light_theme, context.snapshot.status);
    let rendered = render_widget_pixels(&context.snapshot.text, width, height, dpi, foreground);
    debug_assert!(pixels_are_premultiplied_bgra(&rendered));
    pixels.copy_from_slice(&rendered);

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

    SelectObject(memory_dc, previous_bitmap);
    let _ = DeleteObject(HGDIOBJ(bitmap.0));
    let _ = DeleteDC(memory_dc);
}

fn render_widget_pixels(
    text: &str,
    width: i32,
    height: i32,
    dpi: u32,
    foreground: [u8; 3],
) -> Vec<u8> {
    if width <= 0 || height <= 0 {
        return Vec::new();
    }
    let mut pixels = vec![0; width as usize * height as usize * 4];
    let padding = scale(BASE_PADDING, dpi);
    let icon_size = scale(BASE_ICON_SIZE, dpi);
    let icon_gap = scale(BASE_ICON_GAP, dpi);
    let text_left = padding + icon_size + icon_gap;
    let lines = widget_text_lines(text);
    for (index, line) in lines.iter().enumerate() {
        let midpoint = height / 2;
        let (top, bottom) = if lines.len() == 1 {
            (0, height)
        } else if index == 0 {
            (0, midpoint)
        } else {
            (midpoint, height)
        };
        let text_width = (width - padding - text_left).max(0) as u32;
        let text_height = (bottom - top).max(0) as u32;
        let mask = rasterize_system_title_text_mask(line, text_width, text_height, dpi);
        blend_text_mask(
            &mut pixels,
            width,
            height,
            text_left,
            top,
            &mask,
            text_width as i32,
            text_height as i32,
            foreground,
        );
    }
    draw_taskbar_icon(&mut pixels, width, height, padding, icon_size);
    pixels
}

unsafe fn create_system_title_font(dpi: u32, prefer_variable: bool) -> HFONT {
    let font_height =
        -((WIDGET_FONT_SIZE_DIP * dpi as f32 / 96.0 * TEXT_SUPERSAMPLE as f32).round() as i32);
    let face_name = if prefer_variable {
        SYSTEM_TITLE_FONT_PRIMARY
    } else {
        SYSTEM_TITLE_FONT_FALLBACK
    };
    let face = to_wide(face_name);
    CreateFontW(
        font_height,
        0,
        0,
        0,
        WIDGET_FONT_WEIGHT,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_TT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        ANTIALIASED_QUALITY,
        u32::from(DEFAULT_PITCH.0 | FF_DONTCARE.0),
        PCWSTR(face.as_ptr()),
    )
}

unsafe fn selected_text_face(dc: HDC) -> String {
    let mut face = [0_u16; 64];
    let copied = GetTextFaceW(dc, Some(&mut face)).max(0) as usize;
    let end = face[..copied.min(face.len())]
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(copied.min(face.len()));
    String::from_utf16_lossy(&face[..end])
}

unsafe fn select_system_title_font(dc: HDC, dpi: u32) -> (HFONT, HGDIOBJ, String) {
    let preferred = create_system_title_font(dpi, true);
    if !preferred.is_invalid() {
        let previous = SelectObject(dc, HGDIOBJ(preferred.0));
        let selected = selected_text_face(dc);
        if selected.eq_ignore_ascii_case(SYSTEM_TITLE_FONT_PRIMARY) {
            return (preferred, previous, selected);
        }
        SelectObject(dc, previous);
        let _ = DeleteObject(HGDIOBJ(preferred.0));
    }

    let fallback = create_system_title_font(dpi, false);
    if fallback.is_invalid() {
        return (fallback, HGDIOBJ::default(), String::new());
    }
    let previous = SelectObject(dc, HGDIOBJ(fallback.0));
    let selected = selected_text_face(dc);
    (fallback, previous, selected)
}

fn measure_system_title_text(text: &str, dpi: u32) -> (i32, i32) {
    if text.is_empty() {
        return (0, 0);
    }
    unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return (0, 0);
        }
        let (font, previous_font, _) = select_system_title_font(dc, dpi);
        if font.is_invalid() {
            let _ = DeleteDC(dc);
            return (0, 0);
        }
        let text_wide = text.encode_utf16().collect::<Vec<_>>();
        let mut measured = SIZE::default();
        let measured_ok = GetTextExtentPoint32W(dc, &text_wide, &mut measured).as_bool();
        SelectObject(dc, previous_font);
        let _ = DeleteObject(HGDIOBJ(font.0));
        let _ = DeleteDC(dc);
        if !measured_ok {
            return (0, 0);
        }
        let supersample = TEXT_SUPERSAMPLE as i32;
        (
            (measured.cx + supersample - 1) / supersample + 2,
            (measured.cy + supersample - 1) / supersample + 2,
        )
    }
}

fn rasterize_system_title_text_mask(text: &str, width: u32, height: u32, dpi: u32) -> Vec<u8> {
    if width == 0 || height == 0 || text.is_empty() {
        return vec![0; (width * height) as usize];
    }
    unsafe {
        let source_width = width * TEXT_SUPERSAMPLE;
        let source_height = height * TEXT_SUPERSAMPLE;
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return vec![0; (width * height) as usize];
        }
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: source_width as i32,
                biHeight: -(source_height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let Ok(bitmap) =
            CreateDIBSection(Some(dc), &bitmap_info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            return vec![0; (width * height) as usize];
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            return vec![0; (width * height) as usize];
        }

        let previous_bitmap = SelectObject(dc, HGDIOBJ(bitmap.0));
        let byte_len = source_width as usize * source_height as usize * 4;
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), byte_len);
        pixels.fill(0);
        let (font, previous_font, _) = select_system_title_font(dc, dpi);
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(0x00ff_ffff));
        let mut text_wide = text.encode_utf16().collect::<Vec<_>>();
        let mut text_rect = RECT {
            left: 0,
            top: 0,
            right: source_width as i32,
            bottom: source_height as i32,
        };
        DrawTextW(
            dc,
            &mut text_wide,
            &mut text_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );

        let mut source = vec![0_u8; source_width as usize * source_height as usize];
        for (coverage, pixel) in source.iter_mut().zip(pixels.chunks_exact(4)) {
            *coverage = pixel[0].max(pixel[1]).max(pixel[2]);
        }
        SelectObject(dc, previous_font);
        SelectObject(dc, previous_bitmap);
        let _ = DeleteObject(HGDIOBJ(font.0));
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        downsample_coverage(&source, source_width, source_height, width, height)
    }
}

#[cfg(test)]
fn resolved_system_title_font_face(dpi: u32) -> String {
    unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return String::new();
        }
        let (font, previous_font, selected) = select_system_title_font(dc, dpi);
        if !font.is_invalid() {
            SelectObject(dc, previous_font);
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
        let _ = DeleteDC(dc);
        selected
    }
}

fn downsample_coverage(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    destination_width: u32,
    destination_height: u32,
) -> Vec<u8> {
    let mut destination = vec![0; (destination_width * destination_height) as usize];
    for destination_y in 0..destination_height {
        let top = destination_y * source_height / destination_height;
        let bottom = ((destination_y + 1) * source_height / destination_height).max(top + 1);
        for destination_x in 0..destination_width {
            let left = destination_x * source_width / destination_width;
            let right = ((destination_x + 1) * source_width / destination_width).max(left + 1);
            let mut alpha_sum = 0_u32;
            let mut count = 0_u32;
            for source_y in top..bottom.min(source_height) {
                for source_x in left..right.min(source_width) {
                    alpha_sum += source[(source_y * source_width + source_x) as usize] as u32;
                    count += 1;
                }
            }
            destination[(destination_y * destination_width + destination_x) as usize] =
                ((alpha_sum + count / 2) / count.max(1)) as u8;
        }
    }
    destination
}

#[allow(clippy::too_many_arguments)]
fn blend_text_mask(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    left: i32,
    top: i32,
    mask: &[u8],
    mask_width: i32,
    mask_height: i32,
    foreground: [u8; 3],
) {
    for mask_y in 0..mask_height {
        let target_y = top + mask_y;
        if target_y < 0 || target_y >= height {
            continue;
        }
        for mask_x in 0..mask_width {
            let target_x = left + mask_x;
            if target_x < 0 || target_x >= width {
                continue;
            }
            let coverage = mask[(mask_y * mask_width + mask_x) as usize];
            if coverage == 0 {
                continue;
            }
            let index = ((target_y * width + target_x) * 4) as usize;
            let alpha = coverage as u16;
            pixels[index] = ((foreground[2] as u16 * alpha + 127) / 255) as u8;
            pixels[index + 1] = ((foreground[1] as u16 * alpha + 127) / 255) as u8;
            pixels[index + 2] = ((foreground[0] as u16 * alpha + 127) / 255) as u8;
            pixels[index + 3] = coverage;
        }
    }
}

fn pixels_are_premultiplied_bgra(pixels: &[u8]) -> bool {
    pixels
        .chunks_exact(4)
        .all(|pixel| pixel[0] <= pixel[3] && pixel[1] <= pixel[3] && pixel[2] <= pixel[3])
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

fn draw_taskbar_icon(pixels: &mut [u8], width: i32, height: i32, left: i32, icon_size: i32) {
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
            let source_alpha = icon[source_index + 3] as u16;
            if source_alpha == 0 {
                continue;
            }
            let inverse_alpha = 255 - source_alpha;
            for channel in 0..3 {
                let source = icon[source_index + channel] as u16;
                let target_channel = target_index + (2 - channel);
                let destination = pixels[target_channel] as u16;
                let blended = (source * source_alpha + destination * inverse_alpha + 127) / 255;
                // The layered DIB is BGRA while the icon and theme colors are RGBA/RGB.
                pixels[target_channel] = blended as u8;
            }
            let destination_alpha = pixels[target_index + 3] as u16;
            pixels[target_index + 3] =
                (source_alpha + (destination_alpha * inverse_alpha + 127) / 255) as u8;
        }
    }
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
        detach_from_taskbar(hwnd, context);
        log_layout_change(context, "visible=false reason=setting-hidden".to_string());
        let _ = ShowWindow(hwnd, SW_HIDE);
        return;
    }

    let scan_widgets = context.last_widgets_scan.map_or(true, |last_scan| {
        last_scan.elapsed() >= WIDGETS_SCAN_INTERVAL
    });
    let automation = scan_widgets
        .then_some(context.automation.as_ref())
        .flatten();
    let Some(mut taskbar) = locate_taskbar(context.anchor_hwnd, automation, scan_widgets) else {
        detach_from_taskbar(hwnd, context);
        log_layout_change(
            context,
            "visible=false reason=taskbar-unavailable".to_string(),
        );
        let _ = ShowWindow(hwnd, SW_HIDE);
        return;
    };
    if scan_widgets {
        context.last_widgets_scan = Some(Instant::now());
        context.cached_widgets_enabled = taskbar.widgets_enabled;
    } else {
        taskbar.widgets_enabled = context.cached_widgets_enabled;
    }
    let widgets_button_rect = resolve_widgets_button_rect(
        taskbar.widgets_button_rect,
        context.cached_widgets_button_rect,
        taskbar.rect,
        taskbar.widgets_enabled,
    );
    taskbar.widgets_button_rect = widgets_button_rect;
    context.cached_widgets_button_rect = widgets_button_rect;
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
    let (width, height) = desired_size(&context.snapshot.text, dpi);
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
            if embed_in_taskbar(hwnd, context, taskbar.hwnd) {
                let Some((client_x, client_y)) =
                    screen_to_taskbar_client(taskbar.hwnd, screen_x, screen_y)
                else {
                    detach_from_taskbar(hwnd, context);
                    log_layout_change(
                        context,
                        format!(
                            "placement={} action=fallback-floating reason=taskbar-coordinate-conversion-failed",
                            placement_name
                        ),
                    );
                    position_floating_widget(hwnd, context, &taskbar, width, height, dpi);
                    return;
                };
                apply_widget_region(hwnd, width, height, dpi);
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOP),
                    client_x,
                    client_y,
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
                        "visible=true placement={} surface=taskbar-child background=per-pixel-transparent edge={:?} dpi={} parent={:?} taskbar=({},{},{},{}) screen_bounds=({},{},{},{}) client_origin=({},{}) center_hit={:?} owns_center={}",
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
                        client_x,
                        client_y,
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

    if foreground_window_covers_monitor(taskbar.monitor.rcMonitor) {
        detach_from_taskbar(hwnd, context);
        log_layout_change(
            context,
            format!(
                "visible=false reason=foreground-fullscreen-fallback edge={:?} monitor=({},{},{},{})",
                taskbar.edge,
                taskbar.monitor.rcMonitor.left,
                taskbar.monitor.rcMonitor.top,
                taskbar.monitor.rcMonitor.right,
                taskbar.monitor.rcMonitor.bottom,
            ),
        );
        let _ = ShowWindow(hwnd, SW_HIDE);
        return;
    }

    detach_from_taskbar(hwnd, context);
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

    let left_edge = taskbar.rect.left + margin;
    let task_list_left = taskbar.task_list_rect.map(|rect| rect.left);
    let right_limit = task_list_left.unwrap_or(taskbar.rect.right - margin);
    let x = taskbar
        .widgets_button_rect
        .filter(|rect| rect.right > taskbar.rect.left && rect.left < right_limit)
        .map(|rect| rect.right + margin)
        .unwrap_or_else(|| {
            if taskbar.widgets_enabled != Some(false)
                && task_list_left.is_some()
                && right_limit - left_edge >= width + margin * 2
            {
                right_limit - margin - width
            } else {
                left_edge
            }
        });
    if x < left_edge || x + width + margin > right_limit {
        return None;
    }
    Some((x, taskbar.rect.top + (taskbar_height - height) / 2))
}

fn taskbar_child_style(style: u32) -> u32 {
    (style & !WS_POPUP.0) | WS_CHILD.0 | WS_CLIPSIBLINGS.0
}

fn popup_style(style: u32) -> u32 {
    (style & !(WS_CHILD.0 | WS_CLIPSIBLINGS.0)) | WS_POPUP.0
}

unsafe fn embed_in_taskbar(hwnd: HWND, context: &mut WindowContext, parent: HWND) -> bool {
    let parent_raw = parent.0 as isize;
    let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
    if context.taskbar_parent == Some(parent)
        && GetWindowLongPtrW(hwnd, GWLP_HWNDPARENT) == parent_raw
        && style & WS_CHILD.0 != 0
    {
        return true;
    }

    let _ = ShowWindow(hwnd, SW_HIDE);
    SetWindowLongPtrW(hwnd, GWL_STYLE, taskbar_child_style(style) as isize);
    let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (ex_style & !WS_EX_TOPMOST.0) as isize);
    if SetParent(hwnd, Some(parent)).is_ok()
        && GetWindowLongPtrW(hwnd, GWLP_HWNDPARENT) == parent_raw
    {
        context.taskbar_parent = Some(parent);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
        return true;
    }

    let _ = SetParent(hwnd, None);
    SetWindowLongPtrW(hwnd, GWL_STYLE, popup_style(style) as isize);
    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (ex_style | WS_EX_TOPMOST.0) as isize);
    context.taskbar_parent = None;
    false
}

unsafe fn detach_from_taskbar(hwnd: HWND, context: &mut WindowContext) {
    let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
    if context.taskbar_parent.is_none()
        && GetWindowLongPtrW(hwnd, GWLP_HWNDPARENT) == 0
        && style & WS_CHILD.0 == 0
    {
        return;
    }

    let _ = ShowWindow(hwnd, SW_HIDE);
    let _ = SetParent(hwnd, None);
    SetWindowLongPtrW(hwnd, GWL_STYLE, popup_style(style) as isize);
    let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (ex_style | WS_EX_TOPMOST.0) as isize);
    context.taskbar_parent = None;
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

unsafe fn screen_to_taskbar_client(
    taskbar: HWND,
    screen_x: i32,
    screen_y: i32,
) -> Option<(i32, i32)> {
    let mut point = POINT {
        x: screen_x,
        y: screen_y,
    };
    ScreenToClient(taskbar, &mut point)
        .as_bool()
        .then_some((point.x, point.y))
}

fn rect_covers_monitor(window: RECT, monitor: RECT, tolerance: i32) -> bool {
    window.left <= monitor.left + tolerance
        && window.top <= monitor.top + tolerance
        && window.right >= monitor.right - tolerance
        && window.bottom >= monitor.bottom - tolerance
}

fn should_hide_for_fullscreen(
    shell_reports_fullscreen: bool,
    foreground_style: u32,
    foreground_rect: RECT,
    monitor: RECT,
) -> bool {
    shell_reports_fullscreen
        || (foreground_style & WS_MAXIMIZE.0 == 0
            && rect_covers_monitor(foreground_rect, monitor, 2))
}

unsafe fn foreground_window_covers_monitor(monitor: RECT) -> bool {
    let foreground = GetForegroundWindow();
    if foreground.0.is_null() || IsIconic(foreground).as_bool() {
        return false;
    }
    let mut rect = RECT::default();
    if GetWindowRect(foreground, &mut rect).is_err() {
        return false;
    }
    let style = GetWindowLongPtrW(foreground, GWL_STYLE) as u32;
    let shell_reports_fullscreen = matches!(
        SHQueryUserNotificationState(),
        Ok(state)
            if state == QUNS_BUSY
                || state == QUNS_RUNNING_D3D_FULL_SCREEN
                || state == QUNS_PRESENTATION_MODE
    );
    should_hide_for_fullscreen(shell_reports_fullscreen, style, rect, monitor)
}

fn log_layout_change(context: &mut WindowContext, detail: String) {
    if context.last_layout_log != detail {
        log::info!("WINDOWS_QUOTA_WIDGET_LAYOUT {detail}");
        context.last_layout_log = detail;
    }
}

fn desired_size(text: &str, dpi: u32) -> (i32, i32) {
    let lines = widget_text_lines(text);
    let height = scale(base_height_for_text(text), dpi);
    let padding = scale(BASE_PADDING, dpi);
    let icon_size = scale(BASE_ICON_SIZE, dpi);
    let icon_gap = scale(BASE_ICON_GAP, dpi);
    let text_width = lines
        .iter()
        .map(|line| measure_system_title_text(line, dpi).0)
        .max()
        .unwrap_or_default()
        .max(scale(MIN_TEXT_WIDTH, dpi));

    (
        (padding * 2 + icon_size + icon_gap + text_width).min(scale(MAX_WIDTH, dpi)),
        height,
    )
}

unsafe fn locate_taskbar(
    anchor_hwnd: HWND,
    automation: Option<&IUIAutomation>,
    inspect_widgets: bool,
) -> Option<TaskbarPlacement> {
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
    let (widgets_enabled, widgets_button_rect) = if inspect_widgets {
        let widgets_enabled = windows_widgets_enabled().ok();
        let widgets_button_rect = if widgets_enabled == Some(false) {
            None
        } else {
            taskbar_widgets_button_rect(automation, taskbar)
        };
        (widgets_enabled, widgets_button_rect)
    } else {
        (None, None)
    };
    Some(TaskbarPlacement {
        hwnd: taskbar,
        rect,
        tray_rect,
        task_list_rect,
        widgets_enabled,
        widgets_button_rect,
        monitor,
        edge,
        auto_hide,
        revealed,
    })
}

unsafe fn taskbar_widgets_button_rect(
    automation: Option<&IUIAutomation>,
    taskbar: HWND,
) -> Option<RECT> {
    let automation = automation?;
    let taskbar_element = automation.ElementFromHandle(taskbar).ok()?;
    let condition = automation.CreateTrueCondition().ok()?;
    let descendants = taskbar_element
        .FindAll(TreeScope_Descendants, &condition)
        .ok()?;
    let count = descendants.Length().ok()?;
    for index in 0..count {
        let Ok(element) = descendants.GetElement(index) else {
            continue;
        };
        let Ok(automation_id) = element.CurrentAutomationId() else {
            continue;
        };
        if automation_id != "WidgetsButton" {
            continue;
        }
        let rect = element.CurrentBoundingRectangle().ok()?;
        if rect.right > rect.left && rect.bottom > rect.top {
            return Some(rect);
        }
    }
    None
}

fn rect_overlaps_taskbar(rect: RECT, taskbar: RECT) -> bool {
    rect.right > taskbar.left
        && rect.left < taskbar.right
        && rect.bottom > taskbar.top
        && rect.top < taskbar.bottom
}

fn resolve_widgets_button_rect(
    detected: Option<RECT>,
    cached: Option<RECT>,
    taskbar: RECT,
    widgets_enabled: Option<bool>,
) -> Option<RECT> {
    if widgets_enabled == Some(false) {
        return None;
    }
    detected.or_else(|| cached.filter(|rect| rect_overlaps_taskbar(*rect, taskbar)))
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

pub(crate) fn windows_widgets_enabled() -> Result<bool, String> {
    unsafe {
        let mut value = 1_u32;
        let mut size = size_of::<u32>() as u32;
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced"),
            w!("TaskbarDa"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        );
        if status == ERROR_SUCCESS {
            return Ok(value != 0);
        }
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(true);
        }
        Err(format!(
            "Failed to read the Windows Widgets taskbar setting: {status:?}"
        ))
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
        base_height_for_text, blend_text_mask, desired_size, embedded_screen_position,
        left_screen_position, measure_system_title_text, pixels_are_premultiplied_bgra,
        popup_style, rasterize_system_title_text_mask, rect_covers_monitor, render_widget_pixels,
        resolve_widgets_button_rect, resolved_system_title_font_face, scale,
        should_hide_for_fullscreen, taskbar_child_style, taskbar_edge, visible_taskbar_thickness,
        widget_foreground, widget_text_lines, TaskbarEdge, TaskbarPlacement, WindowsWidgetStatus,
        BASE_ICON_GAP, BASE_ICON_SIZE, BASE_PADDING, BASE_SINGLE_LINE_HEIGHT,
        SYSTEM_TITLE_FONT_FALLBACK, SYSTEM_TITLE_FONT_PRIMARY,
    };
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::MONITORINFO;
    use windows::Win32::UI::WindowsAndMessaging::{
        WS_CHILD, WS_CLIPSIBLINGS, WS_MAXIMIZE, WS_POPUP,
    };

    #[test]
    fn taskbar_embedding_switches_between_popup_and_child_styles() {
        let child = taskbar_child_style(WS_POPUP.0);
        assert_eq!(child & WS_POPUP.0, 0);
        assert_ne!(child & WS_CHILD.0, 0);
        assert_ne!(child & WS_CLIPSIBLINGS.0, 0);

        let popup = popup_style(child);
        assert_ne!(popup & WS_POPUP.0, 0);
        assert_eq!(popup & WS_CHILD.0, 0);
        assert_eq!(popup & WS_CLIPSIBLINGS.0, 0);
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
        assert!(should_hide_for_fullscreen(false, 0, monitor, monitor));
        assert!(!should_hide_for_fullscreen(
            false,
            WS_MAXIMIZE.0,
            monitor,
            monitor,
        ));
        assert!(should_hide_for_fullscreen(
            true,
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
            widgets_button_rect: None,
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
            widgets_button_rect: None,
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
            widgets_button_rect: None,
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
    fn left_widget_moves_after_the_windows_widgets_button() {
        let mut placement = TaskbarPlacement {
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
            widgets_button_rect: Some(RECT {
                left: 6,
                top: 1040,
                right: 220,
                bottom: 1080,
            }),
            monitor: MONITORINFO::default(),
            edge: TaskbarEdge::Bottom,
            auto_hide: false,
            revealed: true,
        };

        assert_eq!(
            left_screen_position(&placement, 100, 26, 96),
            Some((226, 1047))
        );

        placement
            .widgets_button_rect
            .as_mut()
            .expect("widgets")
            .right = 450;
        assert_eq!(left_screen_position(&placement, 100, 26, 96), None);
    }

    #[test]
    fn transient_widgets_button_detection_failure_keeps_the_last_valid_bounds() {
        let taskbar = RECT {
            left: 0,
            top: 1040,
            right: 1920,
            bottom: 1080,
        };
        let cached = RECT {
            left: 6,
            top: 1040,
            right: 220,
            bottom: 1080,
        };
        assert_eq!(
            resolve_widgets_button_rect(None, Some(cached), taskbar, Some(true)),
            Some(cached)
        );

        assert_eq!(
            resolve_widgets_button_rect(None, Some(cached), taskbar, Some(false)),
            None
        );

        let moved_taskbar = RECT {
            left: 1920,
            top: 1040,
            right: 3840,
            bottom: 1080,
        };
        assert_eq!(
            resolve_widgets_button_rect(None, Some(cached), moved_taskbar, Some(true)),
            None
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
        let source = include_str!("windows_taskbar_widget.rs");
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
}
