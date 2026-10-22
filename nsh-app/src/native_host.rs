//! 原生后端宿主（默认）：NSH 跑在本平台自己的后端上，不经过 GPUI。
//! - Linux：nsh-platform-linux（GTK 4 控件 + GSK 绘制，Pango 排版在引擎线程）。帧指标来自窗口的
//!   GdkFrameClock：有 paint 阶段的帧算一次绘制（update 阶段结束 → after-paint），present 用
//!   FrameTimings 的 presentation_time（合成器不提供时退回 after-paint 时刻）。
//! - macOS：nsh-platform-macos（AppKit / Core Animation）。没有帧 profiler，补丁在 surface 里把
//!   “改变了屏幕内容的一次主线程工作”（应用一批 / 动画帧）通过 `nsh_core::diag::frame` 报出来，
//!   draw 与 present 都按这次工作的起止计。

use std::path::PathBuf;
use std::sync::Arc;

use bench_core::Driver;

#[cfg(target_os = "linux")]
pub fn run(driver: Arc<Driver>, bundle_path: PathBuf, registry: nsh_core::ui::Registry) {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::rc::Rc;
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};

    use nsh_platform_linux::gtk4::{self as gtk, gdk, glib, prelude::*};
    use nsh_platform_linux::{App, AppOptions, AppView, Bundle, Root};

    nsh_platform_linux::set_ui_sets(vec![nsh_ui_mobile::registry(), registry]);
    let app = gtk::Application::builder()
        .application_id("org.example.nsh.perfbench")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    let started = Cell::new(false);
    app.connect_activate(move |app| {
        if started.replace(true) {
            return;
        }
        let window = gtk::ApplicationWindow::builder().application(app).title("bench · NSH").default_width(1600).default_height(1000).build();
        let view = AppView::new(AppOptions::default());
        window.set_child(Some(&view.widget()));
        if crate::rustui_mode() {
            view.load(App::Rust(Root::of::<crate::rustui::BenchRoot>()));
        } else {
            // SAFETY: 本工程自己的构建产物
            view.load(App::Js(unsafe { Bundle::read_trusted(&bundle_path) }.expect("bundle")));
        }
        window.present();

        // GDK 时间是 CLOCK_MONOTONIC 微秒，与 Instant 同一时钟：取一对对应点换算
        let (base_us, base) = (glib::monotonic_time(), Instant::now());
        let at = move |us: i64| if us >= base_us { base + Duration::from_micros((us - base_us) as u64) } else { base };

        // 帧时钟：update 结束 → layout → paint（GSK 渲染）→ after-paint
        let clock = window.frame_clock().expect("frame clock");
        let frame = Rc::new(Cell::new((None::<Instant>, None::<Instant>, false)));
        let f = frame.clone();
        clock.connect_before_paint(move |_| f.set((Some(Instant::now()), None, false)));
        let f = frame.clone();
        clock.connect_update(move |_| {
            let (w, _, p) = f.get();
            f.set((w, Some(Instant::now()), p));
        });
        let f = frame.clone();
        clock.connect_paint(move |_| {
            let (w, d, _) = f.get();
            f.set((w, d, true));
        });
        // 绘制过的帧：等合成器回报 presentation_time 后记 present
        let presented: Rc<RefCell<VecDeque<(i64, Instant)>>> = Rc::default();
        let d = driver.clone();
        let p = presented.clone();
        clock.connect_after_paint(move |clock| {
            let (work, draw, painted) = frame.get();
            if !painted {
                return;
            }
            let end = Instant::now();
            let draw = draw.or(work).unwrap_or(end);
            d.metrics.on_draw(draw, end, work);
            if let Some(t) = clock.current_timings() {
                if t.refresh_interval() > 0 {
                    d.metrics.set_refresh(Duration::from_micros(t.refresh_interval() as u64));
                }
                p.borrow_mut().push_back((t.frame_counter(), end));
            }
        });
        let d = driver.clone();
        let app = app.clone();
        let clock = clock.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            {
                let mut q = presented.borrow_mut();
                while let Some(&(n, painted_at)) = q.front() {
                    let timings: Option<gdk::FrameTimings> = clock.timings(n);
                    match timings {
                        Some(t) if t.is_complete() => {
                            let shown = t.presentation_time();
                            d.metrics.on_present(if shown > 0 { at(shown) } else { painted_at });
                        }
                        // 历史已被丢弃或迟迟未完成：按绘制完成时刻计
                        None => d.metrics.on_present(painted_at),
                        Some(_) if painted_at.elapsed() > Duration::from_millis(500) => d.metrics.on_present(painted_at),
                        Some(_) => break,
                    }
                    q.pop_front();
                }
            }
            if d.done.load(Ordering::Relaxed) {
                let app = app.clone();
                glib::timeout_add_local_once(Duration::from_millis(300), move || app.quit());
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
        // 窗口持有视图（及其引擎）
        let keep = RefCell::new(Some(view));
        window.connect_destroy(move |_| {
            if let Some(v) = keep.borrow_mut().take() {
                v.stop();
            }
        });
    });
    app.run_with_args::<&str>(&[]);
}

#[cfg(target_os = "macos")]
pub fn run(driver: Arc<Driver>, bundle_path: PathBuf, registry: nsh_core::ui::Registry) {
    use std::sync::OnceLock;
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};

    use nsh_platform_macos::objc2::MainThreadMarker;
    use nsh_platform_macos::objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSWindow, NSWindowStyleMask};
    use nsh_platform_macos::objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
    use nsh_platform_macos::{App, AppOptions, AppView, Bundle, Root};

    static DRIVER: OnceLock<Arc<Driver>> = OnceLock::new();
    let _ = DRIVER.set(driver.clone());
    nsh_core::diag::set_frame_hook(|start, end| {
        if let Some(d) = DRIVER.get() {
            d.metrics.on_draw(start, end, Some(start));
            d.metrics.on_present(end);
        }
    });
    nsh_platform_macos::set_ui_sets(vec![nsh_ui_mobile::registry(), registry]);

    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let rect = NSRect::new(NSPoint::new(100.0, 100.0), NSSize::new(1600.0, 1000.0));
    let style = NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Resizable | NSWindowStyleMask::Miniaturizable;
    // SAFETY: 主线程上创建窗口
    let window = unsafe { NSWindow::initWithContentRect_styleMask_backing_defer(mtm.alloc(), rect, style, NSBackingStoreType::Buffered, false) };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str("bench · NSH"));
    let view = AppView::new(mtm, AppOptions::default());
    window.setContentView(Some(&view.view()));
    if crate::rustui_mode() {
        view.load(App::Rust(Root::of::<crate::rustui::BenchRoot>()));
    } else {
        // SAFETY: 本工程自己的构建产物
        view.load(App::Js(unsafe { Bundle::read_trusted(&bundle_path) }.expect("bundle")));
    }
    window.makeKeyAndOrderFront(None);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    // 压测结束后退出（驱动线程在后台推进）
    std::thread::spawn(move || {
        while !driver.done.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(50));
        }
        // 报告由驱动线程在 done 之后写出；嵌入式宿主没有 run_app 的退出流程，直接结束进程
        std::thread::sleep(Duration::from_millis(500));
        std::process::exit(0);
    });
    let _keep = (view, window, Instant::now());
    app.run();
}

/// Windows：nsh-platform-windows（Win32、Direct2D、DirectWrite）。帧指标来自 nsh-003 补丁接上的
/// `nsh_core::diag` 帧钩子：每次 `WM_PAINT`（绘制整棵树 + `Present(1)`）算一帧；present 之后到
/// 显示的节奏由外部 PresentMon 采集（scripts/run-windows.ps1、scripts/presentmon_join.py）
#[cfg(windows)]
pub fn run(driver: Arc<Driver>, bundle_path: PathBuf, registry: nsh_core::ui::Registry) {
    nsh_platform_windows::set_ui_sets(vec![nsh_ui_mobile::registry(), registry]);
    // 与 nsh_platform_windows::run_app 相同：窗口与消息循环在专门的 UI 线程上，线程结束时释放
    // 图形设备等线程局部资源（放到进程退出时释放会得到异常的退出码）
    let ui = std::thread::Builder::new().name("nsh-ui".into()).stack_size(8 << 20).spawn(move || win::run(driver, bundle_path));
    if !ui.is_ok_and(|t| t.join().is_ok()) {
        std::process::exit(1);
    }
}

#[cfg(windows)]
mod win {
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, OnceLock};
    use std::time::Duration;

    use bench_core::Driver;
    use nsh_platform_windows::windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
    use nsh_platform_windows::windows::Win32::Graphics::Gdi::{
        DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
    };
    use nsh_platform_windows::windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
    use nsh_platform_windows::windows::Win32::System::Threading::GetCurrentThreadId;
    use nsh_platform_windows::windows::Win32::UI::HiDpi::{
        AdjustWindowRectExForDpi, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI,
        SetProcessDpiAwarenessContext,
    };
    use nsh_platform_windows::windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, HWND_TOPMOST, MONITORINFOF_PRIMARY, MSG, PostThreadMessageW, SWP_NOACTIVATE, SWP_SHOWWINDOW,
        SetForegroundWindow, SetWindowPos, TranslateMessage, WINDOW_EX_STYLE, WM_QUIT, WS_OVERLAPPEDWINDOW,
    };
    use nsh_platform_windows::windows::core::BOOL;
    use nsh_platform_windows::{App, AppOptions, AppView, Bundle, Root};

    const W: f64 = 1600.0;
    const H: f64 = 1000.0;

    /// 一块显示器：刷新率、是否主屏、有效 DPI、工作区（虚拟桌面像素坐标，去掉任务栏）
    struct Screen {
        name: String,
        hz: u32,
        primary: bool,
        dpi: u32,
        work: RECT,
    }

    fn screens() -> Vec<Screen> {
        unsafe extern "system" fn collect(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
            unsafe { (*(data.0 as *mut Vec<HMONITOR>)).push(h) };
            true.into()
        }
        let mut hs: Vec<HMONITOR> = Vec::new();
        unsafe {
            let _ = EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut hs as *mut _ as isize));
        }
        hs.into_iter()
            .filter_map(|h| unsafe {
                let mut mi = MONITORINFOEXW::default();
                mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
                if !GetMonitorInfoW(h, &mut mi as *mut _ as *mut MONITORINFO).as_bool() {
                    return None;
                }
                let mut dm = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
                let _ = EnumDisplaySettingsW(nsh_platform_windows::windows::core::PCWSTR(mi.szDevice.as_ptr()), ENUM_CURRENT_SETTINGS, &mut dm);
                let (mut dx, mut dy) = (96, 96);
                let _ = GetDpiForMonitor(h, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
                let n = mi.szDevice.iter().position(|&c| c == 0).unwrap_or(mi.szDevice.len());
                Some(Screen {
                    name: String::from_utf16_lossy(&mi.szDevice[..n]),
                    hz: dm.dmDisplayFrequency,
                    primary: mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                    dpi: dx,
                    work: mi.monitorInfo.rcWork,
                })
            })
            .collect()
    }

    /// 窗口放到显示器 `m` 工作区中央，客户区 1600×1000 点（按 DPI 换算成像素），置顶以免被遮挡
    fn place(hwnd: HWND, m: &Screen, shown: bool) {
        // 移过去之后按窗口实际的 DPI（WM_DPICHANGED 已更新），之前按目标屏
        let dpi = if shown { unsafe { GetDpiForWindow(hwnd) } } else { 0 };
        let dpi = if dpi != 0 { dpi } else { m.dpi };
        let px = |v: f64| (v * dpi as f64 / 96.0).round() as i32;
        let mut r = RECT { left: 0, top: 0, right: px(W), bottom: px(H) };
        unsafe {
            let _ = AdjustWindowRectExForDpi(&mut r, WS_OVERLAPPEDWINDOW, false, WINDOW_EX_STYLE(0), dpi);
        }
        let (ww, wh) = (r.right - r.left, r.bottom - r.top);
        let RECT { left, top, right, bottom } = m.work;
        let (x, y) = (left + ((right - left - ww) / 2).max(0), top + ((bottom - top - wh) / 2).max(0));
        unsafe {
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, ww, wh, if shown { SWP_SHOWWINDOW } else { SWP_NOACTIVATE });
        }
    }

    pub(super) fn run(driver: Arc<Driver>, bundle_path: PathBuf) {
        static DRIVER: OnceLock<Arc<Driver>> = OnceLock::new();
        let _ = DRIVER.set(driver.clone());
        nsh_core::diag::set_frame_hook(|start, end| {
            // 窗口出现时、还没应用任何引擎批次的空白绘制不算帧（首帧 = 有内容的第一帧）
            if nsh_core::diag::get(nsh_core::diag::C::ui_batches) == 0 {
                return;
            }
            if let Some(d) = DRIVER.get() {
                d.metrics.on_draw(start, end, Some(start));
                d.metrics.on_present(end);
            }
        });
        // 与 run_app 相同的进程设置：per-monitor DPI（v2）、OLE 套间
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let _ = OleInitialize(None);
        }
        let options = AppOptions { app_id: "org.example.nsh.perfbench".into(), title: "bench · NSH".into(), size: (W as i32, H as i32), ..AppOptions::default() };
        let view = AppView::new(None, options).expect("window");
        let hwnd = view.hwnd();
        let all = screens();
        for m in &all {
            eprintln!("[bench] screen {} {}Hz primary={} dpi {}", m.name, m.hz, m.primary, m.dpi);
        }
        // 刷新率最高的显示器，并列取主屏
        let screen = all.iter().max_by_key(|m| (m.hz, m.primary));
        if let Some(m) = screen {
            place(hwnd, m, false);
            if m.hz > 0 {
                driver.metrics.set_refresh(Duration::from_secs_f64(1.0 / m.hz as f64));
            }
            eprintln!("[bench] window on {} ({}Hz)", m.name, m.hz);
        }
        if crate::rustui_mode() {
            view.load(App::Rust(Root::of::<crate::rustui::BenchRoot>()));
        } else {
            view.load(App::Js(unsafe { Bundle::read_trusted(&bundle_path) }.expect("bundle")));
        }
        view.show();
        if let Some(m) = screen {
            // 移屏后 DPI 可能变了，按窗口实际的 DPI 再摆一次
            place(hwnd, m, true);
        }
        unsafe {
            let _ = SetForegroundWindow(hwnd);
        }
        let tid = unsafe { GetCurrentThreadId() };
        std::thread::spawn(move || {
            while !driver.done.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(50));
            }
            std::thread::sleep(Duration::from_millis(300));
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        });
        let mut msg = MSG::default();
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        view.stop();
        drop(view);
        unsafe { OleUninitialize() };
    }
}
