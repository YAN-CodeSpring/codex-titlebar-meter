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
            ANTIALIASED_QUALITY, BeginPaint, BitBlt, ClientToScreen, CreateFontW, CreateSolidBrush,
            DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteObject,
            DrawTextW, EndPaint, FF_DONTCARE, FW_NORMAL, FillRect, GdiFlush, GetDC,
            GetMonitorInfoW, GetPixel, HGDIOBJ, InvalidateRect, MONITOR_DEFAULTTONEAREST,
            MONITORINFO, MonitorFromWindow, OUT_DEFAULT_PRECIS, PAINTSTRUCT, ReleaseDC, SRCCOPY,
            SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
        },
    },
    System::{
        LibraryLoader::GetModuleHandleW,
        Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
        Threading::{
            CreateMutexW, GetCurrentThreadId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
    },
    UI::{
        HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
            SetProcessDpiAwarenessContext,
        },
        Input::{Ime::ImmDisableIME, KeyboardAndMouse::ReleaseCapture},
        WindowsAndMessaging::{
            AppendMenuW, CREATESTRUCTW, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CreatePopupMenu,
            CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW, EnumWindows, GW_OWNER,
            GWL_EXSTYLE, GWL_STYLE, GWLP_HWNDPARENT, GWLP_USERDATA, GetClientRect, GetCursorPos,
            GetForegroundWindow, GetMessageW, GetWindow, GetWindowLongPtrW, GetWindowRect,
            GetWindowThreadProcessId, HCURSOR, HTCAPTION, HWND_MESSAGE, HWND_TOP, IDC_ARROW,
            IsIconic, IsWindow, IsWindowVisible, IsZoomed, LWA_ALPHA, LoadCursorW, MA_NOACTIVATE,
            MF_CHECKED, MF_POPUP, MF_STRING, MSG, PostMessageW, PostQuitMessage, RegisterClassW,
            SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SendMessageW, SetForegroundWindow,
            SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow,
            TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTALIGN, TrackPopupMenu, TranslateMessage,
            WM_DESTROY, WM_ERASEBKGND, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_MOUSEACTIVATE,
            WM_NCCREATE, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NULL, WM_PAINT, WM_RBUTTONUP,
            WM_TIMER, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW, WS_POPUP, WS_THICKFRAME,
        },
    },
};

use crate::settings::{Corners, Settings, Theme};

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
    theme: Theme,
    transparent: bool,
    glass: bool,
    corners: Corners,
    layered_glass: bool,
    alpha_failed: bool,
    last_glass_frame: Option<GlassFrame>,
    last_solid_frame: Option<GlassFrame>,
    ambient: COLORREF,
    dark: bool,
    menu_open: bool,
    next_theme_check: Instant,
    locale: AppLocale,
    next_locale_check: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowMode {
    Normal,
    Maximized,
    Fullscreen,
}

#[derive(Clone, PartialEq, Eq)]
struct GlassFrame {
    width: i32,
    height: i32,
    dpi: u32,
    snapshot: UsageSnapshot,
    accent: COLORREF,
    locale: AppLocale,
    dark: bool,
    ambient: COLORREF,
    glass: bool,
    transparent: bool,
    corners: Corners,
    pointer: Option<(i32, i32)>,
    stale: bool,
}

unsafe impl Send for AppState {}

static STATE: OnceLock<Mutex<AppState>> = OnceLock::new();
static PALETTES: [Palette; 7] = [
    Palette::blue(),
    Palette::green(),
    Palette::purple(),
    Palette {
        accent: rgb(30, 180, 200),
    },
    Palette {
        accent: rgb(240, 140, 45),
    },
    Palette {
        accent: rgb(235, 100, 165),
    },
    Palette {
        accent: rgb(130, 140, 155),
    },
];
static CODEX_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn run() -> Result<()> {
    let Some(_instance) = InstanceMutex::acquire()? else {
        return Ok(());
    };
    unsafe {
        // This thread only displays text and menus. Opt out before creating any
        // windows so third-party IMEs need not load into this tiny companion.
        // This affects only our UI thread, never Codex or other applications.
        ImmDisableIME(GetCurrentThreadId());
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
            theme: saved_settings.theme,
            transparent: saved_settings.transparent,
            glass: saved_settings.glass,
            corners: saved_settings.corners,
            layered_glass: false,
            alpha_failed: false,
            last_glass_frame: None,
            last_solid_frame: None,
            ambient: rgb(127, 127, 127),
            dark: system_is_dark(),
            menu_open: false,
            next_theme_check: Instant::now(),
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
            class_name.as_ptr(),
            window_name.as_ptr(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            ptr::null_mut(),
            ptr::null_mut(),
            GetModuleHandleW(ptr::null()),
            ptr::null_mut(),
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
        bail!(
            "Register watcher failed: {}",
            std::io::Error::last_os_error()
        );
    }
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            ptr::null(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
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
        state.layered_glass = false;
        state.alpha_failed = false;
        state.last_glass_frame = None;
        state.target = ptr::null_mut();
    }
    Ok(())
}

unsafe extern "system" fn watcher_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
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
                show_settings_menu(hwnd);
                return 0;
            }
            if let Some(target) = caption_target() {
                ReleaseCapture();
                SendMessageW(target, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
            }
            0
        }
        WM_LBUTTONDBLCLK => {
            let mut client = empty_rect();
            GetClientRect(hwnd, &mut client);
            if (lparam as u16) as i32
                >= client.right - (24 * GetDpiForWindow(hwnd).max(96) as i32 / 96)
            {
                return 0;
            }
            if let Some(target) = caption_target() {
                SendMessageW(target, WM_NCLBUTTONDBLCLK, HTCAPTION as usize, 0);
            }
            0
        }
        WM_RBUTTONUP => {
            show_settings_menu(hwnd);
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
    (!state.target.is_null() && state.window_mode != WindowMode::Fullscreen).then_some(state.target)
}

fn track_codex_window() {
    let Some(state_lock) = STATE.get() else {
        return;
    };
    let mut state = match state_lock.lock() {
        Ok(state) => state,
        Err(_) => return,
    };
    if state.menu_open {
        return;
    }
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

    if now >= state.next_theme_check {
        state.ambient =
            sample_codex_color(bounds, unsafe { GetDpiForWindow(target) }.max(96), overlay)
                .unwrap_or_else(|| theme_colors(system_is_dark()).background);
        state.dark = match state.theme {
            Theme::Light => false,
            Theme::Dark => true,
            Theme::Auto => color_is_dark(state.ambient),
        };
        state.next_theme_check = now + LOCALE_CHECK_INTERVAL;
    }
    let glass = state.glass;
    let dual = state.snapshot.primary.is_some() && state.snapshot.weekly.is_some();
    let layered = !state.alpha_failed
        && (glass || state.transparent || state.corners == Corners::Rounded || dual);
    let change_layered_mode = layered != state.layered_glass;
    let dpi = unsafe { GetDpiForWindow(target) }.max(96);
    let scale = dpi as f32 / 96.0;
    let dual_window = state.snapshot.primary.is_some() && state.snapshot.weekly.is_some();
    let placement = overlay_placement(bounds, mode, scale, dual_window);
    let solid_changed = placement.as_ref().is_some_and(|rect| {
        let frame = solid_frame(&state, rect.right - rect.left, rect.bottom - rect.top, dpi);
        state.last_solid_frame.as_ref() != Some(&frame)
    });
    let initialize_solid = !layered && (change_layered_mode || state.last_solid_frame.is_none());
    drop(state);
    let Some(placement) = placement else {
        unsafe { ShowWindow(overlay, SW_HIDE) };
        return;
    };

    let owner_changed = unsafe { GetWindow(overlay, GW_OWNER) } != target;
    if owner_changed {
        unsafe {
            SetWindowLongPtrW(overlay, GWLP_HWNDPARENT, target as isize);
        }
    }
    unsafe {
        let visible = IsWindowVisible(overlay) != 0;
        let mut previous = empty_rect();
        GetWindowRect(overlay, &mut previous);
        if change_layered_mode {
            // Reset only when changing alpha modes, never on an idle timer tick.
            reset_layered_window(overlay);
            if let Some(mut state) = STATE.get().and_then(|state| state.lock().ok()) {
                state.layered_glass = layered;
                state.last_glass_frame = None;
                state.last_solid_frame = None;
            }
        }
        if initialize_solid {
            SetLayeredWindowAttributes(overlay, 0, 255, LWA_ALPHA);
        }
        if !visible || owner_changed || !same_rect(previous, placement) {
            SetWindowPos(
                overlay,
                HWND_TOP,
                placement.left,
                placement.top,
                placement.right - placement.left,
                placement.bottom - placement.top,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        if layered {
            if !render_glass(overlay) {
                // Allocation/compositor failures leave a usable solid meter.
                reset_layered_window(overlay);
                SetLayeredWindowAttributes(overlay, 0, 255, LWA_ALPHA);
                if let Some(mut state) = STATE.get().and_then(|state| state.lock().ok()) {
                    state.alpha_failed = true;
                    state.layered_glass = false;
                    state.last_glass_frame = None;
                    state.last_solid_frame = None;
                }
                InvalidateRect(overlay, ptr::null(), 0);
            }
        } else if solid_changed || !visible || change_layered_mode {
            InvalidateRect(overlay, ptr::null(), 0);
        }
    }
}

fn same_rect(left: RECT, right: RECT) -> bool {
    left.left == right.left
        && left.top == right.top
        && left.right == right.right
        && left.bottom == right.bottom
}

fn solid_frame(state: &AppState, width: i32, height: i32, dpi: u32) -> GlassFrame {
    GlassFrame {
        width,
        height,
        dpi,
        snapshot: state.snapshot.clone(),
        accent: PALETTES[state.palette_index].accent,
        locale: state.locale,
        dark: state.dark,
        ambient: 0,
        glass: false,
        transparent: false,
        corners: state.corners,
        pointer: None,
        stale: snapshot_is_stale(&state.snapshot, Local::now().timestamp()),
    }
}

unsafe fn reset_layered_window(hwnd: HWND) {
    let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style & !(WS_EX_LAYERED as isize));
    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_LAYERED as isize);
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
        if !is_main_window_candidate(style, extended_style) || !GetWindow(hwnd, GW_OWNER).is_null()
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

    let (snapshot, accent, locale, dark, transparent, glass, corners, alpha_failed) = STATE
        .get()
        .and_then(|state| state.lock().ok())
        .map(|state| {
            (
                state.snapshot.clone(),
                PALETTES[state.palette_index].accent,
                state.locale,
                state.dark,
                state.transparent,
                state.glass,
                state.corners,
                state.alpha_failed,
            )
        })
        .unwrap_or((
            UsageSnapshot::default(),
            PALETTES[0].accent,
            AppLocale::English,
            true,
            false,
            false,
            Corners::Rounded,
            false,
        ));

    if !alpha_failed
        && (glass
            || transparent
            || corners == Corners::Rounded
            || (snapshot.primary.is_some() && snapshot.weekly.is_some()))
    {
        EndPaint(hwnd, &paint);
        render_glass(hwnd);
        return;
    }
    let colors = theme_colors(dark);
    let surface = crate::glass::Surface::new(client.right, client.bottom);
    let draw_dc = surface.as_ref().map_or(dc, |surface| surface.dc);
    let background = CreateSolidBrush(colors.background);
    FillRect(draw_dc, &client, background);
    DeleteObject(background as HGDIOBJ);
    draw_contents(
        draw_dc, client, scale, &snapshot, accent, locale, colors, false, false, false,
    );
    if let Some(surface) = surface.as_ref() {
        BitBlt(
            dc,
            0,
            0,
            client.right,
            client.bottom,
            surface.dc,
            0,
            0,
            SRCCOPY,
        );
    }
    EndPaint(hwnd, &paint);
    if let Some(mut state) = STATE.get().and_then(|state| state.lock().ok()) {
        state.last_solid_frame = Some(GlassFrame {
            width: client.right,
            height: client.bottom,
            dpi,
            stale: snapshot_is_stale(&snapshot, Local::now().timestamp()),
            snapshot,
            accent,
            locale,
            dark,
            ambient: 0,
            glass: false,
            transparent: false,
            corners: state.corners,
            pointer: None,
        });
    }
}

unsafe fn draw_contents(
    dc: *mut c_void,
    client: RECT,
    scale: f32,
    snapshot: &UsageSnapshot,
    accent: COLORREF,
    locale: AppLocale,
    colors: ThemeColors,
    transparent: bool,
    glass: bool,
    mask: bool,
) {
    // Small CJK GDI fonts may select bitmap strikes despite AA_QUALITY. Draw
    // outline-sized glyphs first, then resolve fractional coverage ourselves.
    const FACTOR: i32 = 3;
    let width = client.right - client.left;
    let height = client.bottom - client.top;
    let high = width
        .checked_mul(FACTOR)
        .zip(height.checked_mul(FACTOR))
        .and_then(|(width, height)| crate::glass::Surface::new(width, height));
    let low = crate::glass::Surface::new(width, height);
    if let (Some(mut high), Some(mut low)) = (high, low) {
        let high_client = RECT {
            left: 0,
            top: 0,
            right: high.width,
            bottom: high.height,
        };
        if !transparent && !glass && !mask {
            let brush = CreateSolidBrush(colors.background);
            FillRect(high.dc, &high_client, brush);
            DeleteObject(brush as HGDIOBJ);
        }
        draw_contents_native(
            high.dc,
            high_client,
            scale * FACTOR as f32,
            snapshot,
            accent,
            locale,
            colors,
            transparent,
            glass,
            mask,
        );
        GdiFlush();
        low.pixels().copy_from_slice(&crate::glass::downsample_rgb(
            high.pixels(),
            width as usize,
            height as usize,
            FACTOR as usize,
        ));
        BitBlt(
            dc,
            client.left,
            client.top,
            width,
            height,
            low.dc,
            0,
            0,
            SRCCOPY,
        );
    } else {
        draw_contents_native(
            dc,
            client,
            scale,
            snapshot,
            accent,
            locale,
            colors,
            transparent,
            glass,
            mask,
        );
    }
}

unsafe fn draw_contents_native(
    dc: *mut c_void,
    client: RECT,
    scale: f32,
    snapshot: &UsageSnapshot,
    mut accent: COLORREF,
    locale: AppLocale,
    mut colors: ThemeColors,
    _transparent: bool,
    glass: bool,
    mask: bool,
) {
    if mask {
        accent = rgb(255, 255, 255);
        colors = ThemeColors {
            background: 0,
            text: accent,
            muted: accent,
            track: accent,
        };
    }
    let (font_name, logical_height) = match locale {
        AppLocale::Chinese => ("Microsoft YaHei UI", 13.0_f32),
        AppLocale::English => ("Segoe UI", 12.0_f32),
    };
    let font_height = -((logical_height * scale).round() as i32);
    let face = wide(font_name);
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
        // Grayscale antialiasing works with both solid and per-pixel alpha.
        ANTIALIASED_QUALITY.into(),
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
        SetTextColor(dc, colors.muted);
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
        let stale_brush = CreateSolidBrush(colors.track);
        FillRect(dc, &stale_bar, stale_brush);
        DeleteObject(stale_brush as HGDIOBJ);
    } else {
        match (&snapshot.primary, &snapshot.weekly) {
            (Some(primary), Some(weekly)) => {
                let cards = crate::glass::module_rects(width, client.bottom, scale, true);
                draw_metric(
                    dc,
                    RECT {
                        left: 0,
                        top: 0,
                        right: cards[0].right,
                        bottom: text_bottom,
                    },
                    primary,
                    locale,
                    accent,
                    colors,
                    scale,
                    mask,
                );
                draw_metric(
                    dc,
                    RECT {
                        left: cards[1].left,
                        top: 0,
                        right: content_width,
                        bottom: text_bottom,
                    },
                    weekly,
                    locale,
                    accent,
                    colors,
                    scale,
                    mask,
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
                colors,
                scale,
                mask,
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
                SetTextColor(dc, colors.muted);
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
    if !glass {
        let button = CreateSolidBrush(if mask {
            rgb(255, 255, 255)
        } else {
            colors.background
        });
        FillRect(dc, &dot, button);
        DeleteObject(button as HGDIOBJ);
    }
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
}

unsafe fn render_glass(hwnd: HWND) -> bool {
    let mut client = empty_rect();
    GetClientRect(hwnd, &mut client);
    let mut window = empty_rect();
    GetWindowRect(hwnd, &mut window);
    let dpi = GetDpiForWindow(hwnd).max(96);
    let mut pointer = POINT { x: 0, y: 0 };
    let cursor_valid = GetCursorPos(&mut pointer) != 0;
    let pointer = if cursor_valid
        && pointer.x >= window.left
        && pointer.x < window.right
        && pointer.y >= window.top
        && pointer.y < window.bottom
    {
        Some((
            (pointer.x - window.left) / 4 * 4,
            (pointer.y - window.top) / 4 * 4,
        ))
    } else {
        None
    };
    let Some(state_lock) = STATE.get() else {
        return false;
    };
    let frame = {
        let Ok(state) = state_lock.lock() else {
            return false;
        };
        let frame = GlassFrame {
            width: client.right,
            height: client.bottom,
            dpi,
            snapshot: state.snapshot.clone(),
            accent: PALETTES[state.palette_index].accent,
            locale: state.locale,
            dark: state.dark,
            ambient: if state.glass { state.ambient } else { 0 },
            glass: state.glass,
            transparent: state.transparent,
            corners: state.corners,
            pointer: if state.glass { pointer } else { None },
            stale: snapshot_is_stale(&state.snapshot, Local::now().timestamp()),
        };
        if state.last_glass_frame.as_ref() == Some(&frame) {
            return true;
        }
        frame
    };
    let Some(mut color) = crate::glass::Surface::new(frame.width, frame.height) else {
        return false;
    };
    let Some(mut mask) = crate::glass::Surface::new(frame.width, frame.height) else {
        return false;
    };
    let scale = dpi as f32 / 96.0;
    let colors = theme_colors(frame.dark);
    draw_contents(
        color.dc,
        client,
        scale,
        &frame.snapshot,
        frame.accent,
        frame.locale,
        colors,
        frame.transparent,
        frame.glass || !frame.transparent,
        false,
    );
    draw_contents(
        mask.dc,
        client,
        scale,
        &frame.snapshot,
        frame.accent,
        frame.locale,
        colors,
        frame.transparent,
        frame.glass || !frame.transparent,
        true,
    );
    GdiFlush();
    let dual = frame.snapshot.primary.is_some() && frame.snapshot.weekly.is_some() && !frame.stale;
    let (mut pixels, coverage) = crate::glass::panels(
        frame.width,
        frame.height,
        scale,
        dual,
        frame.pointer,
        crate::glass::PanelStyle {
            rounded: frame.corners == Corners::Rounded,
            glass: frame.glass,
            transparent: frame.transparent,
            dark: frame.dark,
            ambient: frame.ambient,
            solid: colors.background,
        },
    );
    crate::glass::clip_content(color.pixels(), mask.pixels(), &coverage);
    crate::glass::composite(&mut pixels, color.pixels(), mask.pixels());
    color.pixels().copy_from_slice(&pixels);
    let success = color.present(hwnd);
    if success {
        if let Ok(mut state) = state_lock.lock() {
            state.last_glass_frame = Some(frame);
        }
    }
    success
}

unsafe fn draw_metric(
    dc: *mut c_void,
    rect: RECT,
    window: &LimitWindow,
    locale: AppLocale,
    accent: COLORREF,
    colors: ThemeColors,
    scale: f32,
    mask: bool,
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
    SetTextColor(dc, colors.text);
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
    let track = CreateSolidBrush(colors.track);
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
    let brush = CreateSolidBrush(if mask {
        rgb(255, 255, 255)
    } else {
        quota_color(window.remaining_percent, accent)
    });
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

#[cfg(test)]
const TRANSPARENT_KEY: COLORREF = rgb(1, 2, 3);

#[derive(Clone, Copy)]
struct ThemeColors {
    background: COLORREF,
    text: COLORREF,
    muted: COLORREF,
    track: COLORREF,
}

fn theme_colors(dark: bool) -> ThemeColors {
    if dark {
        ThemeColors {
            background: rgb(31, 31, 31),
            text: rgb(224, 224, 224),
            muted: rgb(160, 160, 160),
            track: rgb(70, 70, 70),
        }
    } else {
        ThemeColors {
            background: rgb(255, 255, 255),
            text: rgb(30, 30, 30),
            muted: rgb(95, 95, 95),
            track: rgb(210, 210, 210),
        }
    }
}

fn color_is_dark(color: COLORREF) -> bool {
    let red = color & 255;
    let green = (color >> 8) & 255;
    let blue = (color >> 16) & 255;
    red * 299 + green * 587 + blue * 114 < 128_000
}

fn system_is_dark() -> bool {
    let mut light: u32 = 1;
    let mut size = size_of::<u32>() as u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize").as_ptr(),
            wide("AppsUseLightTheme").as_ptr(),
            RRF_RT_REG_DWORD,
            ptr::null_mut(),
            &mut light as *mut _ as *mut c_void,
            &mut size,
        );
    }
    light == 0
}

// Sample only seven pixels of the visible title bar, to the left of our overlay.
// Nothing is captured, stored or sent. The majority avoids isolated text/icons.
fn sample_codex_color(bounds: RECT, dpi: u32, overlay: HWND) -> Option<COLORREF> {
    let scale = dpi as f32 / 96.0;
    unsafe {
        let dc = GetDC(ptr::null_mut());
        if dc.is_null() {
            return None;
        }
        let mut samples = Vec::with_capacity(7);
        let mut excluded = empty_rect();
        let exclude_overlay =
            IsWindowVisible(overlay) != 0 && GetWindowRect(overlay, &mut excluded) != 0;
        for offset in [230, 250, 270, 290, 310, 330, 350] {
            let x = bounds.left + (offset as f32 * scale).round() as i32;
            let y = bounds.top + (16.0 * scale).round() as i32;
            if x >= bounds.right - (200.0 * scale) as i32 {
                continue;
            }
            // A narrow window can put these samples beneath our own overlay.
            // Never feed the previous glass/transparent frame back into itself.
            if exclude_overlay
                && x >= excluded.left
                && x < excluded.right
                && y >= excluded.top
                && y < excluded.bottom
            {
                continue;
            }
            let color = GetPixel(dc, x, y);
            if color != u32::MAX {
                samples.push(color);
            }
        }
        ReleaseDC(ptr::null_mut(), dc);
        samples.sort_by_key(|color| {
            (color & 255) * 299 + ((color >> 8) & 255) * 587 + ((color >> 16) & 255) * 114
        });
        (samples.len() >= 3).then(|| samples[samples.len() / 2])
    }
}

fn apply_menu_command(settings: &mut Settings, command: u32) {
    match command {
        1 => settings.theme = Theme::Auto,
        2 => settings.theme = Theme::Light,
        3 => settings.theme = Theme::Dark,
        4 => {
            settings.transparent = false;
            settings.glass = false;
        }
        5 => {
            settings.transparent = true;
            settings.glass = false;
        }
        6 => {
            settings.transparent = false;
            settings.glass = true;
        }
        7 => settings.corners = Corners::Rounded,
        8 => settings.corners = Corners::Square,
        100..=106 => settings.palette_index = (command - 100) as usize,
        _ => {}
    }
}

fn show_settings_menu(hwnd: HWND) {
    let Some(state_lock) = STATE.get() else {
        return;
    };
    let (mut settings, chinese, target) = {
        let Ok(mut state) = state_lock.lock() else {
            return;
        };
        if state.menu_open {
            return;
        }
        state.menu_open = true;
        (
            Settings {
                palette_index: state.palette_index,
                theme: state.theme,
                transparent: state.transparent,
                glass: state.glass,
                corners: state.corners,
            },
            state.locale == AppLocale::Chinese,
            state.target,
        )
    };
    let command = unsafe {
        let menu = CreatePopupMenu();
        let theme = CreatePopupMenu();
        let background = CreatePopupMenu();
        let accents = CreatePopupMenu();
        let corners = CreatePopupMenu();
        for (id, zh, en, checked) in [
            (
                1,
                "自动（跟随 Codex）",
                "Auto (follow Codex)",
                settings.theme == Theme::Auto,
            ),
            (2, "浅色", "Light", settings.theme == Theme::Light),
            (3, "深色", "Dark", settings.theme == Theme::Dark),
        ] {
            AppendMenuW(
                theme,
                MF_STRING | if checked { MF_CHECKED } else { 0 },
                id,
                wide(if chinese { zh } else { en }).as_ptr(),
            );
        }
        for (id, zh, en, checked) in [
            (4, "实色", "Solid", !settings.transparent && !settings.glass),
            (5, "透明", "Transparent", settings.transparent),
            (
                6,
                "液态玻璃（实验）",
                "Liquid glass (experimental)",
                settings.glass,
            ),
        ] {
            AppendMenuW(
                background,
                MF_STRING | if checked { MF_CHECKED } else { 0 },
                id,
                wide(if chinese { zh } else { en }).as_ptr(),
            );
        }
        for (id, zh, en, checked) in [
            (7, "圆角", "Rounded", settings.corners == Corners::Rounded),
            (8, "方角", "Square", settings.corners == Corners::Square),
        ] {
            AppendMenuW(
                corners,
                MF_STRING | if checked { MF_CHECKED } else { 0 },
                id,
                wide(if chinese { zh } else { en }).as_ptr(),
            );
        }
        for (index, (zh, en)) in [
            ("蓝色", "Blue"),
            ("绿色", "Green"),
            ("紫色", "Purple"),
            ("青色", "Cyan"),
            ("橙色", "Orange"),
            ("粉色", "Pink"),
            ("石墨灰", "Graphite"),
        ]
        .iter()
        .enumerate()
        {
            AppendMenuW(
                accents,
                MF_STRING
                    | if settings.palette_index == index {
                        MF_CHECKED
                    } else {
                        0
                    },
                100 + index,
                wide(if chinese { zh } else { en }).as_ptr(),
            );
        }
        for (submenu, zh, en) in [
            (theme, "外观", "Appearance"),
            (background, "背景", "Background"),
            (corners, "形状", "Corners"),
            (accents, "强调色", "Accent color"),
        ] {
            AppendMenuW(
                menu,
                MF_POPUP,
                submenu as usize,
                wide(if chinese { zh } else { en }).as_ptr(),
            );
        }
        let mut position = POINT { x: 0, y: 0 };
        GetCursorPos(&mut position);
        // A foreground menu owner is required for reliable outside-click dismissal.
        SetForegroundWindow(hwnd);
        let result = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTALIGN,
            position.x,
            position.y,
            0,
            hwnd,
            ptr::null(),
        );
        PostMessageW(hwnd, WM_NULL, 0, 0);
        if GetForegroundWindow() == hwnd && IsWindow(target) != 0 {
            SetForegroundWindow(target);
        }
        DestroyMenu(menu);
        result as u32
    };
    apply_menu_command(&mut settings, command);
    if let Ok(mut state) = state_lock.lock() {
        state.menu_open = false;
        state.palette_index = settings.palette_index;
        state.theme = settings.theme;
        state.transparent = settings.transparent;
        state.glass = settings.glass;
        state.corners = settings.corners;
        state.alpha_failed = false;
        state.next_theme_check = Instant::now();
    }
    if command != 0 {
        settings.save();
    }
    track_codex_window();
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
    use windows_sys::Win32::UI::WindowsAndMessaging::LWA_COLORKEY;

    #[test]
    fn transparent_text_is_antialiased_without_an_opaque_backplate() {
        unsafe {
            let rect = RECT {
                left: 0,
                top: 0,
                right: 220,
                bottom: 30,
            };
            let mut color = crate::glass::Surface::new(220, 30).unwrap();
            let mut mask = crate::glass::Surface::new(220, 30).unwrap();
            let snapshot = UsageSnapshot::default();
            let colors = theme_colors(false);
            for (dc, is_mask) in [(color.dc, false), (mask.dc, true)] {
                draw_contents(
                    dc,
                    rect,
                    1.0,
                    &snapshot,
                    PALETTES[0].accent,
                    AppLocale::Chinese,
                    colors,
                    true,
                    false,
                    is_mask,
                );
            }
            GdiFlush();
            let mut pixels = vec![0; 220 * 30];
            crate::glass::composite(&mut pixels, color.pixels(), mask.pixels());
            assert_eq!(pixels[0], 0, "background stays transparent");
            assert_eq!(
                pixels[15 * 220 + 210] >> 24,
                255,
                "settings button remains clickable"
            );
            let text_area = pixels.chunks(220).flat_map(|row| row[..196].iter());
            assert!(text_area.clone().any(|pixel| pixel >> 24 == 255));
            assert!(
                text_area
                    .clone()
                    .any(|pixel| (1..255).contains(&(pixel >> 24))),
                "text edges have grayscale alpha coverage"
            );
            for pixel in text_area {
                let alpha = pixel >> 24;
                for shift in [0, 8, 16] {
                    assert!((pixel >> shift) & 255 <= alpha);
                }
            }
        }
    }

    #[test]
    fn appearance_commands_preserve_independent_preferences() {
        let mut settings = Settings::default();
        apply_menu_command(&mut settings, 106);
        apply_menu_command(&mut settings, 2);
        apply_menu_command(&mut settings, 5);
        assert_eq!(settings.palette_index, 6);
        assert_eq!(settings.theme, Theme::Light);
        assert!(settings.transparent);
        apply_menu_command(&mut settings, 6);
        assert!(settings.glass);
        assert!(!settings.transparent);
        assert_eq!(settings.palette_index, 6);
        apply_menu_command(&mut settings, 8);
        assert_eq!(settings.corners, Corners::Square);
        assert!(settings.glass);
        assert_eq!(settings.palette_index, 6);
        apply_menu_command(&mut settings, 7);
        assert_eq!(settings.corners, Corners::Rounded);
        assert_eq!(settings.theme, Theme::Light);
        apply_menu_command(&mut settings, 5);
        assert!(!settings.glass);
        assert!(settings.transparent);
        apply_menu_command(&mut settings, 0);
        apply_menu_command(&mut settings, 4);
        assert!(!settings.transparent);
        assert_eq!(settings.theme, Theme::Light);
    }

    #[test]
    fn light_and_dark_foregrounds_have_opposite_luminance() {
        for dark in [false, true] {
            let colors = theme_colors(dark);
            assert_eq!(color_is_dark(colors.background), dark);
            assert_eq!(color_is_dark(colors.text), !dark);
            assert_ne!(colors.background, TRANSPARENT_KEY);
        }
    }

    #[test]
    fn watcher_survives_owner_closure_and_overlay_can_be_recovered_repeatedly() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DestroyWindow, PM_REMOVE, PeekMessageW, WM_QUIT,
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
                theme: Theme::Auto,
                transparent: false,
                glass: false,
                corners: Corners::Rounded,
                layered_glass: false,
                alpha_failed: false,
                last_glass_frame: None,
                last_solid_frame: None,
                ambient: rgb(31, 31, 31),
                dark: true,
                menu_open: false,
                next_theme_check: Instant::now(),
                locale: AppLocale::English,
                next_locale_check: Instant::now(),
            };
            for _ in 0..2 {
                // Exercise both directions of the documented layered-window
                // transition on a hidden window, without touching real Codex.
                reset_layered_window(state.overlay);
                {
                    let mut surface = crate::glass::Surface::new(220, 30).unwrap();
                    surface.pixels().copy_from_slice(&crate::glass::material(
                        220,
                        30,
                        1.0,
                        true,
                        rgb(31, 31, 31),
                        None,
                    ));
                    assert!(surface.present(state.overlay));
                }
                reset_layered_window(state.overlay);
                assert_ne!(
                    SetLayeredWindowAttributes(state.overlay, TRANSPARENT_KEY, 255, LWA_COLORKEY),
                    0
                );
                assert_ne!(
                    SetLayeredWindowAttributes(state.overlay, 0, 255, LWA_ALPHA),
                    0
                );
                let owner = CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    ptr::null(),
                    WS_POPUP,
                    0,
                    0,
                    1,
                    1,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    instance,
                    ptr::null_mut(),
                );
                assert!(!owner.is_null());
                SetWindowLongPtrW(state.overlay, GWLP_HWNDPARENT, owner as isize);
                state.target = owner;
                assert_ne!(DestroyWindow(owner), 0);
                assert_eq!(IsWindow(state.overlay), 0, "owner destroys owned popup");
                assert_ne!(IsWindow(watcher), 0, "watcher remains alive");
                let mut message: MSG = std::mem::zeroed();
                assert_eq!(
                    PeekMessageW(&mut message, ptr::null_mut(), WM_QUIT, WM_QUIT, PM_REMOVE),
                    0,
                    "destroying the overlay must not terminate the message loop"
                );
                recover_overlay(&mut state).expect("recover overlay");
                assert_ne!(IsWindow(state.overlay), 0);
                assert!(state.target.is_null());
                assert_eq!(state.palette_index, 2);
            }
            DestroyWindow(state.overlay);
            DestroyWindow(watcher);
            let mut message: MSG = std::mem::zeroed();
            assert_ne!(
                PeekMessageW(&mut message, ptr::null_mut(), WM_QUIT, WM_QUIT, PM_REMOVE),
                0
            );
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
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn fullscreen_requires_borderless_monitor_coverage() {
        let monitor = rect(0, 0, 1920, 1080);
        assert_eq!(
            classify_window(WS_POPUP, false, monitor, monitor),
            WindowMode::Fullscreen
        );
        // Some applications retain their maximized flag while in fullscreen.
        assert_eq!(
            classify_window(WS_POPUP, true, monitor, monitor),
            WindowMode::Fullscreen
        );
        assert_eq!(
            classify_window(WS_CAPTION, false, monitor, monitor),
            WindowMode::Normal
        );
        assert_eq!(
            classify_window(WS_POPUP, false, rect(0, 0, 1200, 800), monitor),
            WindowMode::Normal
        );
        assert_eq!(
            classify_window(WS_POPUP, false, rect(-1920, 0, 1920, 1080), monitor),
            WindowMode::Normal
        );
        assert_eq!(
            classify_window(WS_POPUP, false, rect(-1, -1, 1921, 1081), monitor),
            WindowMode::Fullscreen
        );
    }

    #[test]
    fn maximized_bounds_remove_invisible_borders_and_respect_taskbar() {
        let monitor = rect(0, 0, 1920, 1080);
        let bounds = rect(-8, 32, 1928, 1088);
        assert_eq!(
            classify_window(WS_CAPTION, true, bounds, monitor),
            WindowMode::Maximized
        );
        let visible = intersect_bounds(bounds, rect(0, 40, 1920, 1080)).unwrap();
        let placement = overlay_placement(visible, WindowMode::Maximized, 1.0, false).unwrap();
        assert_eq!(
            (
                placement.left,
                placement.top,
                placement.right,
                placement.bottom
            ),
            (1542, 45, 1762, 75)
        );
    }

    #[test]
    fn placement_scales_on_negative_coordinate_monitors() {
        for scale in [1.0, 1.5, 2.0] {
            for dual in [false, true] {
                let bounds = rect(-2560, -1440, 0, 0);
                let placement =
                    overlay_placement(bounds, WindowMode::Fullscreen, scale, dual).unwrap();
                let expected_width = ((if dual { 460.0 } else { 220.0 }) * scale).round() as i32;
                assert_eq!(placement.top, -1440 + (8.0 * scale).round() as i32);
                assert_eq!(placement.right, -(158.0 * scale).round() as i32);
                assert_eq!(placement.right - placement.left, expected_width);
                assert_eq!(
                    placement.bottom - placement.top,
                    (30.0 * scale).round() as i32
                );
                assert!(placement.left >= bounds.left && placement.bottom <= bounds.bottom);
            }
        }
    }

    #[test]
    fn state_transitions_recalculate_margins_and_preserve_normal_layout() {
        let bounds = rect(100, 200, 1300, 1000);
        for mode in [
            WindowMode::Normal,
            WindowMode::Maximized,
            WindowMode::Fullscreen,
            WindowMode::Normal,
        ] {
            let placement = overlay_placement(bounds, mode, 1.0, false).unwrap();
            assert_eq!(placement.left, 922);
            assert_eq!(
                placement.top,
                if mode == WindowMode::Fullscreen {
                    208
                } else {
                    205
                }
            );
        }
    }

    #[test]
    fn placement_hides_when_there_is_no_safe_space() {
        assert!(
            overlay_placement(rect(0, 0, 500, 800), WindowMode::Fullscreen, 1.0, false).is_none()
        );
        assert!(
            overlay_placement(rect(0, 0, 1200, 35), WindowMode::Fullscreen, 1.0, false).is_none()
        );
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
