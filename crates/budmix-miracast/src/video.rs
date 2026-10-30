//! The cast's picture in a small window of its own.
//!
//! The window draws the cast's `MediaPlayer` through Windows.UI.Composition.
//! Closing or hiding it only hides it: the player, and so the phone's audio,
//! keeps running.

use std::cell::RefCell;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use windows::Foundation::Size;
use windows::Media::Playback::{MediaPlayer, MediaPlayerSurface};
use windows::UI::Composition::Desktop::DesktopWindowTarget;
use windows::UI::Composition::{CompositionBrush, CompositionStretch, Compositor, SpriteVisual};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{BLACK_BRUSH, GetStockObject, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::WinRT::Composition::ICompositorDesktopInterop;
use windows::Win32::System::WinRT::{
    CreateDispatcherQueueController, DQTAT_COM_STA, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW,
    IDC_ARROW, LoadCursorW, MSG, PostMessageW, RegisterClassW, SW_HIDE, SW_SHOWNORMAL,
    SetForegroundWindow, ShowWindow, TranslateMessage, WM_APP, WM_CLOSE, WM_SIZE, WNDCLASSW,
    WS_EX_NOREDIRECTIONBITMAP, WS_OVERLAPPEDWINDOW,
};
use windows::core::{Interface, PCWSTR, Result, w};
use windows_numerics::Vector2;

const WM_ATTACH: u32 = WM_APP + 1;
const WM_DETACH: u32 = WM_APP + 2;
const WM_SHOW: u32 = WM_APP + 3;
const WM_HIDE: u32 = WM_APP + 4;

/// Composition objects owned by the window thread.
struct Scene {
    compositor: Compositor,
    visual: SpriteVisual,
    _target: DesktopWindowTarget,
    pending: Arc<Mutex<Option<MediaPlayer>>>,
    player: Option<MediaPlayer>,
    _surface: Option<MediaPlayerSurface>,
}

thread_local! {
    static SCENE: RefCell<Option<Scene>> = const { RefCell::new(None) };
}

/// The video window. Its own thread runs the window and composition.
pub struct VideoWindow {
    hwnd: Arc<AtomicIsize>,
    pending: Arc<Mutex<Option<MediaPlayer>>>,
}

impl VideoWindow {
    /// Creates the (hidden) window on its own thread.
    pub fn spawn() -> Result<Self> {
        let pending = Arc::new(Mutex::new(None));
        let hwnd = Arc::new(AtomicIsize::new(0));
        let (ready_tx, ready_rx) = mpsc::channel();
        let thread_pending = Arc::clone(&pending);
        let thread_hwnd = Arc::clone(&hwnd);
        thread::Builder::new()
            .name("budmix-video".into())
            .spawn(move || {
                // SAFETY: the window, its class and the composition objects
                // all live on this thread for the rest of the process.
                let result = unsafe { run(thread_pending, &thread_hwnd) };
                if let Err(err) = &result {
                    log::warn!("the video window stopped: {err}");
                }
                let _ = ready_tx.send(result.err());
            })
            .map_err(|err| {
                windows::core::Error::new(windows::Win32::Foundation::E_FAIL, err.to_string())
            })?;
        // `run` reports only failures before the message loop; success shows
        // up as a window handle.
        while hwnd.load(Ordering::SeqCst) == 0 {
            match ready_rx.recv_timeout(std::time::Duration::from_millis(20)) {
                Ok(Some(err)) => return Err(err),
                Ok(None) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(windows::core::Error::new(
                        windows::Win32::Foundation::E_FAIL,
                        "the video window could not be created",
                    ));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        Ok(Self { hwnd, pending })
    }

    /// Shows this player's picture; `visible` also shows the window.
    pub fn attach(&self, player: MediaPlayer, visible: bool) {
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) = Some(player);
        self.post(WM_ATTACH);
        if visible {
            self.post(WM_SHOW);
        }
    }

    /// The cast ended: clears the picture and hides the window.
    pub fn detach(&self) {
        self.post(WM_DETACH);
        self.post(WM_HIDE);
    }

    pub fn show(&self) {
        self.post(WM_SHOW);
    }

    pub fn hide(&self) {
        self.post(WM_HIDE);
    }

    fn post(&self, message: u32) {
        let hwnd = HWND(self.hwnd.load(Ordering::SeqCst) as *mut _);
        // SAFETY: posting to a window owned by the video thread.
        if let Err(err) = unsafe { PostMessageW(Some(hwnd), message, WPARAM(0), LPARAM(0)) } {
            log::warn!("the video window did not take a message: {err}");
        }
    }
}

/// Creates the window and runs its message loop.
///
/// # Safety
/// Must run on a thread of its own that owns the window for good.
unsafe fn run(pending: Arc<Mutex<Option<MediaPlayer>>>, hwnd_out: &AtomicIsize) -> Result<()> {
    // SAFETY: plain Win32 and WinRT calls on this thread.
    unsafe {
        // Composition needs a dispatcher queue on this thread (and an STA).
        let _queue = CreateDispatcherQueueController(DispatcherQueueOptions {
            dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
            threadType: DQTYPE_THREAD_CURRENT,
            apartmentType: DQTAT_COM_STA,
        })?;

        let instance = HINSTANCE(GetModuleHandleW(PCWSTR::null())?.0);
        let class = w!("BudMixVideoWindow");
        let window_class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&window_class);
        let hwnd = CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP,
            class,
            w!("BudMix - phone screen"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            480,
            300,
            None,
            None,
            Some(instance),
            None,
        )?;

        let compositor = Compositor::new()?;
        let interop: ICompositorDesktopInterop = compositor.cast()?;
        let target = interop.CreateDesktopWindowTarget(hwnd, false)?;
        let visual = compositor.CreateSpriteVisual()?;
        visual.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        target.SetRoot(&visual)?;
        SCENE.with(|scene| {
            *scene.borrow_mut() = Some(Scene {
                compositor,
                visual,
                _target: target,
                pending,
                player: None,
                _surface: None,
            });
        });
        hwnd_out.store(hwnd.0 as isize, Ordering::SeqCst);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_ATTACH => {
            SCENE.with(|scene| {
                if let Some(scene) = scene.borrow_mut().as_mut() {
                    if let Err(err) = attach(scene, hwnd) {
                        log::warn!("showing the phone screen failed: {err}");
                    }
                }
            });
            LRESULT(0)
        }
        WM_DETACH => {
            SCENE.with(|scene| {
                if let Some(scene) = scene.borrow_mut().as_mut() {
                    let _ = scene.visual.SetBrush(None::<&CompositionBrush>);
                    scene.player = None;
                    scene._surface = None;
                }
            });
            LRESULT(0)
        }
        WM_SHOW => {
            // SAFETY: our own window.
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
                let _ = SetForegroundWindow(hwnd);
            }
            LRESULT(0)
        }
        // Closing only hides: the audio keeps playing.
        WM_HIDE | WM_CLOSE => {
            // SAFETY: our own window.
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            SCENE.with(|scene| {
                if let Some(scene) = scene.borrow().as_ref() {
                    resize(scene, hwnd);
                }
            });
            LRESULT(0)
        }
        // SAFETY: default handling for everything else.
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn attach(scene: &mut Scene, hwnd: HWND) -> Result<()> {
    let player = scene
        .pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    let Some(player) = player else {
        return Ok(());
    };
    let surface = player.GetSurface(&scene.compositor)?;
    let brush = scene
        .compositor
        .CreateSurfaceBrushWithSurface(&surface.CompositionSurface()?)?;
    brush.SetStretch(CompositionStretch::Uniform)?;
    scene.visual.SetBrush(&brush)?;
    scene.player = Some(player);
    scene._surface = Some(surface);
    resize(scene, hwnd);
    Ok(())
}

fn resize(scene: &Scene, hwnd: HWND) {
    let Some(player) = &scene.player else {
        return;
    };
    let mut rect = RECT::default();
    // SAFETY: our own window.
    if unsafe { GetClientRect(hwnd, &mut rect) }.is_ok() {
        let size = Size {
            Width: (rect.right - rect.left).max(1) as f32,
            Height: (rect.bottom - rect.top).max(1) as f32,
        };
        let _ = player.SetSurfaceSize(size);
    }
}
