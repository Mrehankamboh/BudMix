//! The dock: a small panel that opens from the tray icon, the way a menu bar
//! app's panel opens on macOS. Everyday control happens here; the full window
//! only opens for settings.

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const DOCK_WINDOW: &str = "dock";
/// Logical width, as in the macOS menu bar panels this is modelled on.
pub const DOCK_WIDTH: f64 = 380.0;
const DEFAULT_HEIGHT: f64 = 560.0;
const MIN_HEIGHT: f64 = 200.0;
const MAX_HEIGHT: f64 = 900.0;
/// Logical gap between the dock and the taskbar or screen edge.
const MARGIN: f64 = 12.0;
/// Clicking the tray icon while the dock is open first takes focus away from
/// the dock, which hides it; the click itself must then not reopen it.
const REOPEN_GUARD: Duration = Duration::from_millis(300);
/// Set to `1` to keep the dock open when it loses focus (demos, recordings).
pub const PINNED_ENV: &str = "BUDMIX_DOCK_PINNED";

/// A rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PxRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl PxRect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn right(&self) -> i32 {
        self.x + self.width
    }

    fn bottom(&self) -> i32 {
        self.y + self.height
    }

    fn center(&self) -> (i32, i32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
}

/// Top-left corner for the dock, in physical pixels.
///
/// `anchor` is the tray icon, `work` the monitor minus the taskbar, `size`
/// the dock. The taskbar side is worked out from where the icon sits
/// relative to the work area, so a taskbar on any edge is handled.
pub fn place(anchor: Option<PxRect>, work: PxRect, size: (i32, i32), margin: i32) -> (i32, i32) {
    let (width, height) = size;
    let min_x = work.x + margin;
    let min_y = work.y + margin;
    let max_x = (work.right() - width - margin).max(min_x);
    let max_y = (work.bottom() - height - margin).max(min_y);
    let clamp_x = |x: i32| x.clamp(min_x, max_x);
    let clamp_y = |y: i32| y.clamp(min_y, max_y);

    let Some(icon) = anchor else {
        return (max_x, max_y); // bottom-right, where the tray usually is
    };
    let (cx, cy) = icon.center();
    if cy >= work.bottom() {
        (clamp_x(cx - width / 2), max_y) // taskbar at the bottom
    } else if cy < work.y {
        (clamp_x(cx - width / 2), min_y) // taskbar at the top
    } else if cx < work.x {
        (min_x, clamp_y(cy - height / 2)) // taskbar on the left
    } else if cx >= work.right() {
        (max_x, clamp_y(cy - height / 2)) // taskbar on the right
    } else {
        // The icon is inside the work area (overflow flyout or auto-hidden
        // taskbar): open above it, or below when there is no room.
        let above = icon.y - height - margin;
        let y = if above >= min_y {
            above
        } else {
            icon.bottom() + margin
        };
        (clamp_x(cx - width / 2), clamp_y(y))
    }
}

#[derive(Default)]
pub struct DockState {
    anchor: Mutex<Option<PxRect>>,
    hidden_at: Mutex<Option<Instant>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn create(app: &AppHandle, backdrop: bool) -> tauri::Result<()> {
    app.manage(DockState::default());
    let builder = WebviewWindowBuilder::new(app, DOCK_WINDOW, WebviewUrl::default())
        .title("BudMix")
        .inner_size(DOCK_WIDTH, DEFAULT_HEIGHT)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(true)
        .visible(false);
    let builder = if backdrop {
        use tauri::window::{Effect, EffectsBuilder};
        builder
            .transparent(true)
            .effects(EffectsBuilder::new().effect(Effect::Mica).build())
    } else {
        builder
    };
    builder.build()?;
    Ok(())
}

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(DOCK_WINDOW)
}

/// Tray left-click: open the dock next to the icon, or close it.
pub fn toggle(app: &AppHandle, anchor: Option<PxRect>) {
    if let Some(anchor) = anchor {
        *lock(&app.state::<DockState>().anchor) = Some(anchor);
    }
    let Some(dock) = window(app) else {
        return;
    };
    if dock.is_visible().unwrap_or(false) {
        hide(app);
        return;
    }
    let recently_hidden =
        lock(&app.state::<DockState>().hidden_at).is_some_and(|at| at.elapsed() < REOPEN_GUARD);
    if !recently_hidden {
        show(app);
    }
}

pub fn show(app: &AppHandle) {
    let Some(dock) = window(app) else {
        return;
    };
    if let Err(err) = position(app, &dock, None) {
        log::warn!("could not place the dock: {err}");
    }
    let _ = dock.show();
    let _ = dock.set_focus();
}

pub fn hide(app: &AppHandle) {
    if let Some(dock) = window(app) {
        let _ = dock.hide();
        *lock(&app.state::<DockState>().hidden_at) = Some(Instant::now());
    }
}

/// The dock lost focus: close it like a flyout, unless pinned.
pub fn focus_lost(app: &AppHandle) {
    let pinned = std::env::var(PINNED_ENV).is_ok_and(|value| value == "1");
    if !pinned {
        hide(app);
    }
}

/// The page reports its content height so the dock fits it exactly.
pub fn resize(app: &AppHandle, height: f64) -> tauri::Result<()> {
    let Some(dock) = window(app) else {
        return Ok(());
    };
    let height = height.clamp(MIN_HEIGHT, MAX_HEIGHT);
    // Some platforms (GTK) ignore size changes on a fixed-size window.
    dock.set_resizable(true)?;
    let resized = dock.set_size(LogicalSize::new(DOCK_WIDTH, height));
    dock.set_resizable(false)?;
    resized?;
    if dock.is_visible()? {
        // Place with the requested size: the window may not report it yet.
        let scale = dock.scale_factor()?;
        let size = (
            (DOCK_WIDTH * scale).round() as i32,
            (height * scale).round() as i32,
        );
        position(app, &dock, Some(size))?;
    }
    Ok(())
}

/// Moves the dock next to the tray icon. `size` overrides the window's
/// current size, in physical pixels.
fn position(app: &AppHandle, dock: &WebviewWindow, size: Option<(i32, i32)>) -> tauri::Result<()> {
    let scale = dock.scale_factor()?;
    let size = match size {
        Some(size) => size,
        None => {
            let outer = dock.outer_size()?;
            (outer.width as i32, outer.height as i32)
        }
    };
    let anchor = lock(&app.state::<DockState>().anchor).or_else(|| tray_rect(app));
    let monitor = anchor
        .and_then(|icon| {
            let (x, y) = icon.center();
            app.monitor_from_point(f64::from(x), f64::from(y))
                .ok()
                .flatten()
        })
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(());
    };
    let area = monitor.work_area();
    let work = PxRect::new(
        area.position.x,
        area.position.y,
        area.size.width as i32,
        area.size.height as i32,
    );
    let margin = (MARGIN * scale).round() as i32;
    let (x, y) = place(anchor, work, size, margin);
    dock.set_position(PhysicalPosition::new(x, y))
}

fn tray_rect(app: &AppHandle) -> Option<PxRect> {
    let tray = app.tray_by_id(crate::tray::TRAY_ID)?;
    let rect = tray.rect().ok().flatten()?;
    to_px(&rect, 1.0)
}

/// Converts a Tauri rectangle (usually already physical) to pixels.
pub fn to_px(rect: &tauri::Rect, scale: f64) -> Option<PxRect> {
    let position = rect.position.to_physical::<i32>(scale);
    let size = rect.size.to_physical::<i32>(scale);
    (size.width > 0 && size.height > 0)
        .then(|| PxRect::new(position.x, position.y, size.width, size.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A 1920x1080 screen with a 48 px taskbar at the bottom.
    const WORK: PxRect = PxRect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1032,
    };
    const SIZE: (i32, i32) = (380, 560);

    #[test]
    fn opens_above_a_bottom_taskbar_centred_on_the_icon() {
        let icon = PxRect::new(1700, 1040, 32, 32);
        assert_eq!(place(Some(icon), WORK, SIZE, 12), (1526, 460));
    }

    #[test]
    fn stays_on_screen_near_the_corner() {
        let icon = PxRect::new(1880, 1040, 32, 32);
        assert_eq!(place(Some(icon), WORK, SIZE, 12), (1528, 460));
    }

    #[test]
    fn opens_below_a_top_taskbar() {
        let work = PxRect::new(0, 48, 1920, 1032);
        let icon = PxRect::new(1700, 8, 32, 32);
        assert_eq!(place(Some(icon), work, SIZE, 12), (1526, 60));
    }

    #[test]
    fn opens_beside_a_side_taskbar() {
        let left = PxRect::new(64, 0, 1856, 1080);
        let icon = PxRect::new(16, 1000, 32, 32);
        assert_eq!(place(Some(icon), left, SIZE, 12), (76, 508));

        let right = PxRect::new(0, 0, 1856, 1080);
        let icon = PxRect::new(1872, 1000, 32, 32);
        assert_eq!(place(Some(icon), right, SIZE, 12), (1464, 508));
    }

    #[test]
    fn icon_inside_the_work_area_opens_above_it() {
        let icon = PxRect::new(1600, 900, 32, 32);
        assert_eq!(place(Some(icon), WORK, SIZE, 12), (1426, 328));
        // No room above: open below, kept on screen.
        let icon = PxRect::new(100, 20, 32, 32);
        assert_eq!(place(Some(icon), WORK, SIZE, 12), (12, 64));
    }

    #[test]
    fn unknown_icon_uses_the_bottom_right_corner() {
        assert_eq!(place(None, WORK, SIZE, 12), (1528, 460));
    }

    #[test]
    fn tiny_screens_never_push_the_dock_off_the_top_left() {
        let work = PxRect::new(0, 0, 300, 400);
        assert_eq!(place(None, work, SIZE, 12), (12, 12));
    }
}
