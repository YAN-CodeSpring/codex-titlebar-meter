#![allow(unsafe_op_in_unsafe_fn)]

use std::{
    ffi::c_void,
    mem::size_of,
    path::PathBuf,
    ptr,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use chrono::Local;
use windows_sys::Win32::{
    Foundation::{
        BOOL, COLORREF, CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM,
        LRESULT, POINT, RECT, TRUE, WPARAM,
    },
    Graphics::{
        Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute},
        Gdi::{
            BeginPaint, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER,
            DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW, EndPaint, FF_DONTCARE, FW_NORMAL,
            FillRect, GdiFlush, HGDIOBJ, InvalidateRect, OUT_DEFAULT_PRECIS, PAINTSTRUCT,
            ClientToScreen, GetMonitorInfoW, MONITORINFO, MONITOR_DEFAULTTONEAREST,
            MonitorFromWindow, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
        },
    },
    System::{
        LibraryLoader::GetModuleHandleW,
        Threading::{
            CreateMutexW, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
    },
    UI::{
        HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
            SetProcessDpiAwarenessContext,
        },
        Input::KeyboardAndMouse::ReleaseCapture,
        WindowsAndMessaging::{
            CREATESTRUCTW, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW,
            DispatchMessageW, EnumWindows, GWL_EXSTYLE, GWL_STYLE, GWLP_HWNDPARENT, GWLP_USERDATA,
            GetClientRect, GetMessageW, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
            GW_OWNER, GetForegroundWindow, GetWindow, IsWindow, IsZoomed,
            HCURSOR, HTCAPTION, HWND_MESSAGE, HWND_TOP, IDC_ARROW, IsIconic, IsWindowVisible, LWA_ALPHA,
            LoadCursorW, MA_NOACTIVATE, MSG, PostQuitMessage, RegisterClassW, SW_HIDE,
            SWP_NOACTIVATE, SWP_SHOWWINDOW, SendMessageW, SetLayeredWindowAttributes, SetTimer,
            SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, WM_DESTROY,
            WM_ERASEBKGND, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_MOUSEACTIVATE, WM_NCCREATE,
            WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_PAINT, WM_RBUTTONUP, WM_TIMER, WNDCLASSW,
            WS_CAPTION, WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
            WS_THICKFRAME,
        },
    },
};

use crate::{
    locale::AppLocale,
    model::{LimitWindow, UsageSnapshot},
};

const CLASS_NAME: &str = "ConfigCrate.CodexTitlebarMeter.Overlay";
const WATCHER_CLASS_NAME: &str = "ConfigCrate.CodexTitlebarMeter.Watcher";
const WINDOW_NAME: &str = "Codex Titlebar Meter";
const MUTEX_NAME: &str = "Local\\ConfigCrate.CodexTitlebarMeter.4BC6AD61";
const TRACK_TIMER: usize = 1;
const TRACK_INTERVAL_MS: u32 = 250;
const LOCALE_CHECK_INTERVAL: Duration = Duration::from_secs(2);
const STALE_AFTER_SECONDS: i64 = 120;

#[derive(Clone, Copy)]
struct Palette {
    accent: COLORREF,
}

impl Palette {
    const fn blue() -> Self {
        Self {
            accent: rgb(95, 145, 255),
        }
    }

    const fn green() -> Self {
        Self {
            accent: rgb(64, 196, 126),
        }
    }

    const fn purple() -> Self {
        Self {
            accent: rgb(173, 117, 255),
        }
    }
}

struct AppState {
    overlay: HWND,
    target: HWND,
    window_mode: WindowMode,
    snapshot: UsageSnapshot,
    palette_index: usize,
    locale: AppLocale,
    next_locale_check: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowMode {
    Normal,
    Maximized,
    Fullscreen,
}

unsafe impl Send for AppState {}

static STATE: OnceLock<Mutex<AppState>> = OnceLock::new();
static PALETTES: [Palette; 3] = [Palette::blue(), Palette::green(), Palette::purple()];
static CODEX_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn run() -> Result<()> {
    let Some(_instance) = InstanceMutex::acquire()? else {
        return Ok(());
    };
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    if instance.is_null() {
        bail!(
            "GetModuleHandleW failed: {}",
            std::io::Error::last_os_error()
        );
    }

    let class_name = wide(CLASS_NAME);
    let cursor = unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) };
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
        lpfnWndProc: Some(window_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: ptr::null_mut(),
        hCursor: cursor as HCURSOR,
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        bail!("RegisterClassW failed: {}", std::io::Error::last_os_error());
    }

    let hwnd = create_overlay()?;
    // This message-only window is never owned by Codex. Its timer survives
    // destruction of the owned overlay when Codex closes or replaces a window.
    let watcher = create_watcher()?;
    let saved_settings = crate::settings::Settings::load();

    STATE
        .set(Mutex::new(AppState {
            overlay: hwnd,
            target: ptr::null_mut(),
            window_mode: WindowMode::Normal,
            snapshot: UsageSnapshot::default(),
            palette_index: saved_settings.palette_index % PALETTES.len(),
            locale: AppLocale::detect(),
            next_locale_check: Instant::now() + LOCALE_CHECK_INTERVAL,
        }))
        .map_err(|_| anyhow::anyhow!("application state was already initialized"))?;

    if unsafe { SetTimer(watcher, TRACK_TIMER, TRACK_INTERVAL_MS, None) } == 0 {
        bail!("SetTimer failed: {}", std::io::Error::last_os_error());
    }
    track_codex_window();
    crate::codex::start_worker();

    let mut message: MSG = unsafe { std::mem::zeroed() };
    loop {
        let result = unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) };
        if result == -1 {
            return Err(std::io::Error::last_os_error()).context("GetMessageW failed");
        }
        if result == 0 {
            break;
        }
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

fn create_overlay() -> Result<HWND> {
    let class_name = wide(CLASS_NAME);
    let window_name = wide(WINDOW_NAME);
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
            class_name.as_ptr(), window_name.as_ptr(), WS_POPUP,
            0, 0, 1, 1, ptr::null_mut(), ptr::null_mut(),
            GetModuleHandleW(ptr::null()), ptr::null_mut(),
        )
    };
    if hwnd.is_null() {
        bail!("Create overlay failed: {}", std::io::Error::last_os_error());
    }
    unsafe { SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA) };
    Ok(hwnd)
}

fn create_watcher() -> Result<HWND> {
    let class_name = wide(WATCHER_CLASS_NAME);
    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    let mut class: WNDCLASSW = unsafe { std::mem::zeroed() };
    class.lpfnWndProc = Some(watcher_proc);
    class.hInstance = instance;
    class.lpszClassName = class_name.as_ptr();
    if unsafe { RegisterClassW(&class) } == 0 {
        bail!("Register watcher failed: {}", std::io::Error::last_os_error());
    }
    let hwnd = unsafe {
        CreateWindowExW(
            0, class_name.as_ptr(), ptr::null(), 0,
            0, 0, 0, 0, HWND_MESSAGE, ptr::null_mut(), instance, ptr::null_mut(),
        )
    };
    if hwnd.is_null() {
        bail!("Create watcher failed: {}", std::io::Error::last_os_error());
    }
    Ok(hwnd)
}

fn recover_overlay(state: &mut AppState) -> Result<()> {
    if state.overlay.is_null() || unsafe { IsWindow(state.overlay) } == 0 {
        state.overlay = create_overlay()?;
        state.target = ptr::null_mut();
    }
    Ok(())
}

unsafe extern "system" fn watcher_proc(
    hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_TIMER if wparam == TRACK_TIMER => {
            track_codex_window();
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

struct InstanceMutex(HANDLE);

impl InstanceMutex {
    fn acquire() -> Result<Option<Self>> {
        let name = wide(MUTEX_NAME);
        let handle = unsafe { CreateMutexW(ptr::null(), 1, name.as_ptr()) };
        if handle.is_null() {
            bail!("CreateMutexW failed: {}", std::io::Error::last_os_error());
        }
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe { CloseHandle(handle) };
            return Ok(None);
        }
        Ok(Some(Self(handle)))
    }
}

impl Drop for InstanceMutex {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_NCCREATE => {
            let create = lparam as *const CREATESTRUCTW;
            if !create.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*create).lpCreateParams as isize);
            }
            TRUE as LRESULT
        }
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_LBUTTONDOWN => {
            let x = (lparam as u16) as i32;
            let mut client = empty_rect();
            GetClientRect(hwnd, &mut client);
            let dpi = GetDpiForWindow(hwnd).max(96);
            let settings_width = (24 * dpi as i32 / 96).max(24);
            if x >= client.right - settings_width {
                cycle_palette(hwnd);
                return 0;
            }
            if let Some(target) = caption_target() {
                ReleaseCapture();
                SendMessageW(target, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
            }
            0
        }
        WM_LBUTTONDBLCLK => {
            if let Some(target) = caption_target() {
                SendMessageW(target, WM_NCLBUTTONDBLCLK, HTCAPTION as usize, 0);
            }
            0
        }
        WM_RBUTTONUP => {
            cycle_palette(hwnd);
            0
        }
        WM_DESTROY => {
            // Closing the owner destroys this popup, not the resident watcher.
            // try_lock also avoids re-entrancy if Windows destroys it during a
            // window operation performed while state is being inspected.
            if let Some(mut state) = STATE.get().and_then(|state| state.try_lock().ok())
                && state.overlay == hwnd
            {
                state.overlay = ptr::null_mut();
                state.target = ptr::null_mut();
                CODEX_ACTIVE.store(false, Ordering::Relaxed);
            }
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

// Never hold STATE while forwarding synchronous messages to another process.
fn caption_target() -> Option<HWND> {
    let state = STATE.get()?.lock().ok()?;
    (!state.target.is_null() && state.window_mode != WindowMode::Fullscreen)
        .then_some(state.target)
}

fn track_codex_window() {
    let Some(state_lock) = STATE.get() else {
        return;
    };
    let mut state = match state_lock.lock() {
        Ok(state) => state,
        Err(_) => return,
    };
    if let Err(error) = recover_overlay(&mut state) {
        CODEX_ACTIVE.store(false, Ordering::Relaxed);
        eprintln!("Recreate quota overlay: {error:#}");
        return;
    }
    let overlay = state.overlay;
    let now = Instant::now();
    if now >= state.next_locale_check {
        let locale = AppLocale::detect();
        if locale != state.locale {
            state.locale = locale;
            unsafe { InvalidateRect(state.overlay, ptr::null(), 0) };
        }
        state.next_locale_check = now + LOCALE_CHECK_INTERVAL;
    }
    let target = find_codex_window(state.target);
    state.target = target;
    if target.is_null() || unsafe { IsIconic(target) } != 0 || window_is_cloaked(target) {
        CODEX_ACTIVE.store(false, Ordering::Relaxed);
        drop(state);
        unsafe { ShowWindow(overlay, SW_HIDE) };
        return;
    }

    let Some((mode, bounds)) = window_placement(target) else {
        CODEX_ACTIVE.store(false, Ordering::Relaxed);
        drop(state);
        unsafe { ShowWindow(overlay, SW_HIDE) };
        return;
    };
    state.window_mode = mode;
    CODEX_ACTIVE.store(true, Ordering::Relaxed);

    // Keep the data reader alive while Codex is in the background, but do not
    // repeatedly raise its overlay over whichever application is being used.
    if unsafe { GetForegroundWindow() } != target {
        drop(state);
        unsafe { ShowWindow(overlay, SW_HIDE) };
        return;
    }

    let dpi = unsafe { GetDpiForWindow(target) }.max(96);
    let scale: f32 = dpi as f32 / 96.0_f32;
    let dual_window = state.snapshot.primary.is_some() && state.snapshot.weekly.is_some();
    let placement = overlay_placement(bounds, mode, scale, dual_window);
    drop(state);
    let Some(placement) = placement else {
        unsafe { ShowWindow(overlay, SW_HIDE) };
        return;
    };

    if unsafe { GetWindow(overlay, GW_OWNER) } != target {
        unsafe {
            SetWindowLongPtrW(overlay, GWLP_HWNDPARENT, target as isize);
        }
    }
    unsafe {
        SetWindowPos(
            overlay,
            HWND_TOP,
            placement.left,
            placement.top,
            placement.right - placement.left,
            placement.bottom - placement.top,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        InvalidateRect(overlay, ptr::null(), 0);
    }
}

fn overlay_layout(total_width: i32, scale: f32, dual_window: bool) -> Option<(i32, i32)> {
    let left_reserve = (220.0_f32 * scale).round() as i32;
    let right_reserve = (158.0_f32 * scale).round() as i32;
    let preferred_width = if dual_window { 460.0_f32 } else { 220.0_f32 };
    let preferred_width = (preferred_width * scale).round() as i32;
    let available_width = total_width - left_reserve - right_reserve;
    let minimum_width = (170.0_f32 * scale).round() as i32;
    if available_width < minimum_width {
        return None;
    }
    let width = preferred_width.min(available_width);
    let left = total_width - right_reserve - width;
    Some((left, width))
}

fn overlay_placement(
    bounds: RECT,
    mode: WindowMode,
    scale: f32,
    dual_window: bool,
) -> Option<RECT> {
    let (relative_left, width) = overlay_layout(bounds.right - bounds.left, scale, dual_window)?;
    let top_margin = if mode == WindowMode::Fullscreen {
        8.0
    } else {
        5.0
    };
    let top = bounds.top + (top_margin * scale).round() as i32;
    let height = (30.0 * scale).round() as i32;
    if top + height > bounds.bottom {
        return None;
    }
    Some(RECT {
        left: bounds.left + relative_left,
        top,
        right: bounds.left + relative_left + width,
        bottom: top + height,
    })
}

fn classify_window(style: u32, maximized: bool, bounds: RECT, monitor: RECT) -> WindowMode {
    let covers_monitor = (bounds.left - monitor.left).abs() <= 2
        && (bounds.top - monitor.top).abs() <= 2
        && (bounds.right - monitor.right).abs() <= 2
        && (bounds.bottom - monitor.bottom).abs() <= 2;
    if style & WS_CAPTION != WS_CAPTION && covers_monitor {
        WindowMode::Fullscreen
    } else if maximized {
        WindowMode::Maximized
    } else {
        WindowMode::Normal
    }
}

fn intersect_bounds(bounds: RECT, visible: RECT) -> Option<RECT> {
    let result = RECT {
        left: bounds.left.max(visible.left),
        top: bounds.top.max(visible.top),
        right: bounds.right.min(visible.right),
        bottom: bounds.bottom.min(visible.bottom),
    };
    (result.right > result.left && result.bottom > result.top).then_some(result)
}

fn window_placement(target: HWND) -> Option<(WindowMode, RECT)> {
    unsafe {
        let mut bounds = empty_rect();
        if GetWindowRect(target, &mut bounds) == 0 {
            return None;
        }
        let monitor = MonitorFromWindow(target, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if monitor.is_null() || GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        let mode = classify_window(
            GetWindowLongPtrW(target, GWL_STYLE) as u32,
            IsZoomed(target) != 0,
            bounds,
            info.rcMonitor,
        );
        let visible = match mode {
            WindowMode::Normal => bounds,
            // Maximized window rectangles include invisible resize borders.
            WindowMode::Maximized => intersect_bounds(bounds, info.rcWork)?,
            WindowMode::Fullscreen => {
                let mut client = empty_rect();
                let mut origin = POINT { x: 0, y: 0 };
                if GetClientRect(target, &mut client) == 0
                    || ClientToScreen(target, &mut origin) == 0
                {
                    return None;
                }
                client.left += origin.x;
                client.right += origin.x;
                client.top += origin.y;
                client.bottom += origin.y;
                intersect_bounds(client, info.rcMonitor)?
            }
        };
        Some((mode, visible))
    }
}

fn find_codex_window(previous: HWND) -> HWND {
    // Preserve the selected main window through maximize/fullscreen transitions,
    // minimization and virtual desktop changes instead of attaching to another.
    if !previous.is_null() && is_codex_main_window(previous, true) {
        return previous;
    }
    let foreground = unsafe { GetForegroundWindow() };
    if !foreground.is_null() && is_codex_main_window(foreground, false) {
        return foreground;
    }
    let mut result: HWND = ptr::null_mut();
    unsafe {
        EnumWindows(
            Some(enum_windows_callback),
            (&mut result as *mut HWND) as LPARAM,
        );
    }
    result
}

unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if is_codex_main_window(hwnd, false) {
        *(lparam as *mut HWND) = hwnd;
        return 0;
    }
    TRUE
}

fn is_codex_main_window(hwnd: HWND, known_target: bool) -> bool {
    unsafe {
        if IsWindow(hwnd) == 0 || IsWindowVisible(hwnd) == 0 {
            return false;
        }
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let extended_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if !is_main_window_candidate(style, extended_style)
            || !GetWindow(hwnd, GW_OWNER).is_null()
        {
            return false;
        }
        if !known_target {
            if IsIconic(hwnd) != 0 || window_is_cloaked(hwnd) {
                return false;
            }
            // Do not mistake arbitrary frameless helper windows for the main app.
            let has_frame = style & WS_CAPTION == WS_CAPTION || style & WS_THICKFRAME != 0;
            if !has_frame
                && !window_placement(hwnd).is_some_and(|(mode, _)| mode == WindowMode::Fullscreen)
            {
                return false;
            }
        }
        let Some(path) = process_path_for_window(hwnd) else {
            return false;
        };
        let normalized = path.to_string_lossy().to_ascii_lowercase();
        normalized.contains("\\openai.codex_") && normalized.ends_with("\\app\\chatgpt.exe")
    }
}

fn window_is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked: u32 = 0;
    let result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32,
            (&mut cloaked as *mut u32).cast(),
            size_of::<u32>() as u32,
        )
    };
    result == 0 && cloaked != 0
}

fn is_main_window_candidate(style: u32, extended_style: u32) -> bool {
    style & WS_CHILD == 0 && extended_style & WS_EX_TOOLWINDOW == 0
}

unsafe fn paint(hwnd: HWND) {
    let mut paint: PAINTSTRUCT = std::mem::zeroed();
    let dc = BeginPaint(hwnd, &mut paint);
    if dc.is_null() {
        return;
    }
    let mut client = empty_rect();
    GetClientRect(hwnd, &mut client);
    let dpi = GetDpiForWindow(hwnd).max(96);
    let scale: f32 = dpi as f32 / 96.0_f32;

    let (snapshot, accent, locale) = STATE
        .get()
        .and_then(|state| state.lock().ok())
        .map(|state| {
            (
                state.snapshot.clone(),
                PALETTES[state.palette_index].accent,
                state.locale,
            )
        })
        .unwrap_or((
            UsageSnapshot::default(),
            PALETTES[0].accent,
            AppLocale::English,
        ));

    let background = CreateSolidBrush(rgb(31, 31, 31));
    FillRect(dc, &client, background);
    DeleteObject(background as HGDIOBJ);

    let font_height = -((12.0_f32 * scale).round() as i32);
    let face = wide("Segoe UI");
    let font = CreateFontW(
        font_height,
        0,
        0,
        0,
        FW_NORMAL as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.into(),
        OUT_DEFAULT_PRECIS.into(),
        0,
        0,
        (DEFAULT_PITCH | FF_DONTCARE).into(),
        face.as_ptr(),
    );
    let previous = SelectObject(dc, font as HGDIOBJ);
    SetBkMode(dc, TRANSPARENT as i32);

    let width = client.right - client.left;
    let settings_width = (24.0_f32 * scale).round() as i32;
    let content_width = (width - settings_width).max(1);
    let text_bottom = client.bottom - (5.0_f32 * scale).round() as i32;
    if snapshot_is_stale(&snapshot, Local::now().timestamp()) {
        let status = wide(locale.status_text(crate::model::UsageStatus::Stale));
        let mut status_rect = RECT {
            right: content_width,
            bottom: text_bottom,
            ..client
        };
        SetTextColor(dc, rgb(150, 150, 150));
        DrawTextW(
            dc,
            status.as_ptr(),
            -1,
            &mut status_rect,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );

        let padding = (6.0_f32 * scale).round() as i32;
        let bar_height = (2.0_f32 * scale).round().max(2.0_f32) as i32;
        let stale_bar = RECT {
            left: padding,
            top: text_bottom - bar_height,
            right: content_width - padding,
            bottom: text_bottom,
        };
        let stale_brush = CreateSolidBrush(rgb(92, 92, 92));
        FillRect(dc, &stale_bar, stale_brush);
        DeleteObject(stale_brush as HGDIOBJ);
    } else {
        match (&snapshot.primary, &snapshot.weekly) {
            (Some(primary), Some(weekly)) => {
                let gap = (10.0_f32 * scale).round() as i32;
                let side = ((content_width - gap) / 2).max(1);
                draw_metric(
                    dc,
                    RECT {
                        left: 0,
                        top: 0,
                        right: side,
                        bottom: text_bottom,
                    },
                    primary,
                    locale,
                    accent,
                    scale,
                );
                draw_metric(
                    dc,
                    RECT {
                        left: side + gap,
                        top: 0,
                        right: content_width,
                        bottom: text_bottom,
                    },
                    weekly,
                    locale,
                    accent,
                    scale,
                );
            }
            (Some(window), None) | (None, Some(window)) => draw_metric(
                dc,
                RECT {
                    left: 0,
                    top: 0,
                    right: content_width,
                    bottom: text_bottom,
                },
                window,
                locale,
                accent,
                scale,
            ),
            (None, None) => {
                let status = wide(
                    snapshot
                        .status
                        .map(|status| locale.status_text(status))
                        .unwrap_or_else(|| locale.status_text(crate::model::UsageStatus::Retrying)),
                );
                let mut status_rect = RECT {
                    right: content_width,
                    ..client
                };
                SetTextColor(dc, rgb(150, 150, 150));
                DrawTextW(
                    dc,
                    status.as_ptr(),
                    -1,
                    &mut status_rect,
                    DT_CENTER | DT_SINGLELINE | DT_VCENTER,
                );
            }
        }
    }

    let mut dot = RECT {
        left: client.right - settings_width,
        top: 0,
        right: client.right,
        bottom: client.bottom,
    };
    let dot_text = wide("...");
    SetTextColor(dc, accent);
    DrawTextW(
        dc,
        dot_text.as_ptr(),
        -1,
        &mut dot,
        DT_CENTER | DT_SINGLELINE | DT_VCENTER,
    );

    SelectObject(dc, previous);
    DeleteObject(font as HGDIOBJ);
    GdiFlush();
    EndPaint(hwnd, &paint);
}

unsafe fn draw_metric(
    dc: *mut c_void,
    rect: RECT,
    window: &LimitWindow,
    locale: AppLocale,
    accent: COLORREF,
    scale: f32,
) {
    let padding = (6.0_f32 * scale).round() as i32;
    let text = locale.metric_text(window);
    let text = wide(&text);
    let mut text_rect = RECT {
        left: rect.left + padding,
        top: rect.top,
        right: rect.right - padding,
        bottom: rect.bottom,
    };
    SetTextColor(dc, rgb(224, 224, 224));
    DrawTextW(
        dc,
        text.as_ptr(),
        -1,
        &mut text_rect,
        DT_CENTER | DT_SINGLELINE | DT_VCENTER,
    );

    let bar_height = (2.0_f32 * scale).round().max(2.0_f32) as i32;
    let bar_top = rect.bottom - bar_height;
    let full = RECT {
        left: rect.left + padding,
        top: bar_top,
        right: rect.right - padding,
        bottom: rect.bottom,
    };
    let track = CreateSolidBrush(rgb(61, 61, 61));
    FillRect(dc, &full, track);
    GdiFlush();
    DeleteObject(track as HGDIOBJ);

    let available_width = (full.right - full.left).max(0);
    let fill_width = available_width * window.remaining_percent as i32 / 100;
    let fill = RECT {
        left: full.left,
        top: full.top,
        right: full.left + fill_width,
        bottom: full.bottom,
    };
    let brush = CreateSolidBrush(quota_color(window.remaining_percent, accent));
    FillRect(dc, &fill, brush);
    GdiFlush();
    DeleteObject(brush as HGDIOBJ);
}

fn quota_color(remaining_percent: u8, accent: COLORREF) -> COLORREF {
    match remaining_percent {
        0..=10 => rgb(239, 83, 94),
        11..=20 => rgb(235, 178, 54),
        _ => accent,
    }
}

fn snapshot_is_stale(snapshot: &UsageSnapshot, now_timestamp: i64) -> bool {
    snapshot
        .sampled_at
        .as_ref()
        .is_some_and(|sampled_at| now_timestamp - sampled_at.timestamp() >= STALE_AFTER_SECONDS)
}

const fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    red as u32 | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

const fn empty_rect() -> RECT {
    RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    }
}

pub(crate) fn codex_is_active() -> bool {
    CODEX_ACTIVE.load(Ordering::Relaxed)
}

pub(crate) fn update_snapshot(snapshot: UsageSnapshot) {
    let Some(state) = STATE.get() else { return };
    let hwnd = match state.lock() {
        Ok(mut state) => {
            state.snapshot = snapshot;
            state.overlay
        }
        Err(_) => return,
    };
    if !hwnd.is_null() {
        unsafe { InvalidateRect(hwnd, ptr::null(), 0) };
    }
}

pub(crate) fn mark_usage_connecting() {
    mark_usage_status(crate::model::UsageStatus::Connecting);
}

pub(crate) fn mark_usage_retrying() {
    mark_usage_status(crate::model::UsageStatus::Retrying);
}

fn mark_usage_status(status: crate::model::UsageStatus) {
    let Some(state) = STATE.get() else { return };
    let hwnd = match state.lock() {
        Ok(mut state) => {
            if state.snapshot.primary.is_none() && state.snapshot.weekly.is_none() {
                state.snapshot = match status {
                    crate::model::UsageStatus::Connecting => UsageSnapshot::connecting(),
                    crate::model::UsageStatus::Retrying | crate::model::UsageStatus::Stale => {
                        UsageSnapshot::retrying()
                    }
                };
            } else {
                state.snapshot.status = Some(status);
            }
            state.overlay
        }
        Err(_) => return,
    };
    if !hwnd.is_null() {
        unsafe { InvalidateRect(hwnd, ptr::null(), 0) };
    }
}

fn cycle_palette(hwnd: HWND) {
    let Some(state) = STATE.get() else { return };
    let palette_index = match state.lock() {
        Ok(mut state) => {
            state.palette_index = (state.palette_index + 1) % PALETTES.len();
            state.palette_index
        }
        Err(_) => return,
    };
    crate::settings::Settings { palette_index }.save();
    unsafe {
        InvalidateRect(hwnd, ptr::null(), 0);
    }
}

pub(crate) fn codex_desktop_cli_source() -> Option<PathBuf> {
    let target = STATE.get()?.lock().ok()?.target;
    if target.is_null() {
        return None;
    }
    let app_executable = unsafe { process_path_for_window(target) }?;
    let app_directory = app_executable.parent()?;
    let source = app_directory.join("resources").join("codex.exe");
    source.is_file().then_some(source)
}

unsafe fn process_path_for_window(hwnd: HWND) -> Option<PathBuf> {
    let mut process_id = 0;
    GetWindowThreadProcessId(hwnd, &mut process_id);
    if process_id == 0 {
        return None;
    }
    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
    if process.is_null() {
        return None;
    }
    let mut buffer = [0_u16; 1024];
    let mut length = buffer.len() as u32;
    let ok = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length);
    CloseHandle(process);
    (ok != 0).then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watcher_survives_owner_closure_and_overlay_can_be_recovered_repeatedly() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DestroyWindow, PeekMessageW, PM_REMOVE, WM_QUIT,
        };
        unsafe {
            let instance = GetModuleHandleW(ptr::null());
            let class_name = wide(CLASS_NAME);
            let mut class: WNDCLASSW = std::mem::zeroed();
            class.lpfnWndProc = Some(window_proc);
            class.hInstance = instance;
            class.lpszClassName = class_name.as_ptr();
            assert_ne!(RegisterClassW(&class), 0);
            let watcher = create_watcher().expect("resident watcher");
            assert_ne!(SetTimer(watcher, TRACK_TIMER, TRACK_INTERVAL_MS, None), 0);
            let mut state = AppState {
                overlay: create_overlay().expect("overlay"),
                target: ptr::null_mut(),
                window_mode: WindowMode::Normal,
                snapshot: UsageSnapshot::default(),
                palette_index: 2,
                locale: AppLocale::English,
                next_locale_check: Instant::now(),
            };
            for _ in 0..2 {
                let owner = CreateWindowExW(
                    0, class_name.as_ptr(), ptr::null(), WS_POPUP,
                    0, 0, 1, 1, ptr::null_mut(), ptr::null_mut(), instance, ptr::null_mut(),
                );
                assert!(!owner.is_null());
                SetWindowLongPtrW(state.overlay, GWLP_HWNDPARENT, owner as isize);
                state.target = owner;
                assert_ne!(DestroyWindow(owner), 0);
                assert_eq!(IsWindow(state.overlay), 0, "owner destroys owned popup");
                assert_ne!(IsWindow(watcher), 0, "watcher remains alive");
                let mut message: MSG = std::mem::zeroed();
                assert_eq!(PeekMessageW(&mut message, ptr::null_mut(), WM_QUIT, WM_QUIT, PM_REMOVE), 0,
                    "destroying the overlay must not terminate the message loop");
                recover_overlay(&mut state).expect("recover overlay");
                assert_ne!(IsWindow(state.overlay), 0);
                assert!(state.target.is_null());
                assert_eq!(state.palette_index, 2);
            }
            DestroyWindow(state.overlay);
            DestroyWindow(watcher);
            let mut message: MSG = std::mem::zeroed();
            assert_ne!(PeekMessageW(&mut message, ptr::null_mut(), WM_QUIT, WM_QUIT, PM_REMOVE), 0);
        }
    }

    #[test]
    fn compact_layout_is_right_aligned_and_preserves_menu_space() {
        let (left, width) = overlay_layout(1_200, 1.0, false).expect("layout");
        assert_eq!(width, 220);
        assert_eq!(left, 822);
        assert_eq!(1_200 - left - width, 158);
        assert!(left >= 220);
    }

    #[test]
    fn compact_layout_scales_with_dpi() {
        let (left, width) = overlay_layout(1_800, 1.5, false).expect("layout");
        assert_eq!(width, 330);
        assert_eq!(1_800 - left - width, 237);
    }

    #[test]
    fn dual_window_layout_has_room_for_both_reset_labels() {
        let (left, width) = overlay_layout(1_200, 1.0, true).expect("layout");
        assert_eq!(width, 460);
        assert_eq!(1_200 - left - width, 158);
        assert!(left >= 220);
    }

    #[test]
    fn compact_layout_hides_instead_of_covering_menus() {
        assert_eq!(overlay_layout(500, 1.0, false), None);
    }

    #[test]
    fn codex_main_window_is_an_attachment_candidate() {
        assert!(is_main_window_candidate(WS_CAPTION, 0));
    }

    #[test]
    fn codex_pet_tool_window_is_not_an_attachment_candidate() {
        assert!(!is_main_window_candidate(WS_CAPTION, WS_EX_TOOLWINDOW));
        assert!(!is_main_window_candidate(WS_POPUP, WS_EX_TOOLWINDOW));
    }

    #[test]
    fn borderless_main_window_is_allowed_but_children_are_rejected() {
        assert!(is_main_window_candidate(WS_POPUP, 0));
        assert!(!is_main_window_candidate(WS_CHILD | WS_CAPTION, 0));
    }

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
        RECT { left, top, right, bottom }
    }

    #[test]
    fn fullscreen_requires_borderless_monitor_coverage() {
        let monitor = rect(0, 0, 1920, 1080);
        assert_eq!(classify_window(WS_POPUP, false, monitor, monitor), WindowMode::Fullscreen);
        // Some applications retain their maximized flag while in fullscreen.
        assert_eq!(classify_window(WS_POPUP, true, monitor, monitor), WindowMode::Fullscreen);
        assert_eq!(classify_window(WS_CAPTION, false, monitor, monitor), WindowMode::Normal);
        assert_eq!(classify_window(WS_POPUP, false, rect(0, 0, 1200, 800), monitor), WindowMode::Normal);
        assert_eq!(classify_window(WS_POPUP, false, rect(-1920, 0, 1920, 1080), monitor), WindowMode::Normal);
        assert_eq!(classify_window(WS_POPUP, false, rect(-1, -1, 1921, 1081), monitor), WindowMode::Fullscreen);
    }

    #[test]
    fn maximized_bounds_remove_invisible_borders_and_respect_taskbar() {
        let monitor = rect(0, 0, 1920, 1080);
        let bounds = rect(-8, 32, 1928, 1088);
        assert_eq!(classify_window(WS_CAPTION, true, bounds, monitor), WindowMode::Maximized);
        let visible = intersect_bounds(bounds, rect(0, 40, 1920, 1080)).unwrap();
        let placement = overlay_placement(visible, WindowMode::Maximized, 1.0, false).unwrap();
        assert_eq!((placement.left, placement.top, placement.right, placement.bottom), (1542, 45, 1762, 75));
    }

    #[test]
    fn placement_scales_on_negative_coordinate_monitors() {
        for scale in [1.0, 1.5, 2.0] {
            for dual in [false, true] {
                let bounds = rect(-2560, -1440, 0, 0);
                let placement = overlay_placement(bounds, WindowMode::Fullscreen, scale, dual).unwrap();
                let expected_width = ((if dual { 460.0 } else { 220.0 }) * scale).round() as i32;
                assert_eq!(placement.top, -1440 + (8.0 * scale).round() as i32);
                assert_eq!(placement.right, -(158.0 * scale).round() as i32);
                assert_eq!(placement.right - placement.left, expected_width);
                assert_eq!(placement.bottom - placement.top, (30.0 * scale).round() as i32);
                assert!(placement.left >= bounds.left && placement.bottom <= bounds.bottom);
            }
        }
    }

    #[test]
    fn state_transitions_recalculate_margins_and_preserve_normal_layout() {
        let bounds = rect(100, 200, 1300, 1000);
        for mode in [WindowMode::Normal, WindowMode::Maximized, WindowMode::Fullscreen, WindowMode::Normal] {
            let placement = overlay_placement(bounds, mode, 1.0, false).unwrap();
            assert_eq!(placement.left, 922);
            assert_eq!(placement.top, if mode == WindowMode::Fullscreen { 208 } else { 205 });
        }
    }

    #[test]
    fn placement_hides_when_there_is_no_safe_space() {
        assert!(overlay_placement(rect(0, 0, 500, 800), WindowMode::Fullscreen, 1.0, false).is_none());
        assert!(overlay_placement(rect(0, 0, 1200, 35), WindowMode::Fullscreen, 1.0, false).is_none());
        assert!(intersect_bounds(rect(-100, 0, 0, 100), rect(0, 0, 100, 100)).is_none());
    }

    #[test]
    fn quota_color_warns_only_when_remaining_is_low() {
        let accent = rgb(95, 145, 255);
        assert_eq!(quota_color(21, accent), accent);
        assert_eq!(quota_color(20, accent), rgb(235, 178, 54));
        assert_eq!(quota_color(10, accent), rgb(239, 83, 94));
    }

    #[test]
    fn quota_becomes_stale_after_two_minutes() {
        let mut snapshot = UsageSnapshot::connecting();
        snapshot.sampled_at = Some(Local::now());
        let sampled_at = snapshot.sampled_at.as_ref().unwrap().timestamp();
        assert!(!snapshot_is_stale(&snapshot, sampled_at + 119));
        assert!(snapshot_is_stale(&snapshot, sampled_at + 120));
    }
}
