//! Screen capture.
//!
//! The real implementation uses `xcap`, which works on Windows, macOS and
//! Linux. It is behind the `capture` feature so the crate (and the whole
//! workspace) builds without pulling a capture backend or its system
//! dependencies — useful for CI that only wants to test the pure logic.

use anyhow::Result;
use image::RgbaImage;

/// A rectangular region of the screen, in physical pixels.
#[derive(Debug, Clone, Copy)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Capture a region of the primary monitor.
///
/// # Windows note
/// Wuthering Waves runs elevated, so for capture to see its window Tethys must
/// also run elevated. Display scaling should be 100% or the region maths below
/// needs to account for the DPI factor.
#[cfg(feature = "capture")]
pub fn capture_region(region: Region) -> Result<RgbaImage> {
    use xcap::Monitor;

    let monitors = Monitor::all().map_err(|e| anyhow::anyhow!("enumerating monitors: {e}"))?;
    let monitor = monitors
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no monitor found"))?;

    let shot = monitor
        .capture_image()
        .map_err(|e| anyhow::anyhow!("capturing monitor: {e}"))?;

    // xcap bundles its own `image` version, so convert to this crate's
    // `RgbaImage` through the raw buffer before using our `image::imageops`.
    // Mixing the two `image` versions directly is a compile error.
    let (fw, fh) = (shot.width(), shot.height());
    let full = RgbaImage::from_raw(fw, fh, shot.into_raw())
        .ok_or_else(|| anyhow::anyhow!("captured buffer did not match {fw}x{fh}"))?;

    // Crop to the requested region, clamped to the captured bounds.
    let x = region.x.min(full.width());
    let y = region.y.min(full.height());
    let w = region.width.min(full.width() - x);
    let h = region.height.min(full.height() - y);

    Ok(image::imageops::crop_imm(&full, x, y, w, h).to_image())
}

/// Fallback when the `capture` feature is disabled: returns an error so callers
/// fail loudly rather than silently getting a blank image.
#[cfg(not(feature = "capture"))]
pub fn capture_region(_region: Region) -> Result<RgbaImage> {
    anyhow::bail!("screen capture is disabled; build tethys-scanner with the `capture` feature")
}

/// Substring (case-insensitive) used to identify the game window by title.
pub const GAME_WINDOW_TITLE: &str = "wuthering waves";

/// Capture the Wuthering Waves window as an image.
///
/// Capturing the window directly (rather than the whole monitor) means the
/// returned image's origin is the game's own top-left, so the layout maths in
/// [`crate::layout`] can work in window-relative coordinates and is unaffected
/// by where the window sits or by other windows overlapping it.
///
/// # Windows note
/// The game runs elevated; Tethys must run elevated too or the window will not
/// be enumerable. The returned image is in physical pixels.
#[cfg(feature = "capture")]
pub fn capture_window_image() -> Result<RgbaImage> {
    use xcap::Window;

    let windows = Window::all().map_err(|e| anyhow::anyhow!("enumerating windows: {e}"))?;
    let window = windows
        .into_iter()
        .find(|w| !w.is_minimized() && w.title().to_lowercase().contains(GAME_WINDOW_TITLE))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Wuthering Waves window not found. Is the game running, and is Tethys \
                 elevated? (The game runs as admin.)"
            )
        })?;

    let shot = window
        .capture_image()
        .map_err(|e| anyhow::anyhow!("capturing window: {e}"))?;

    // Convert to this crate's `image` version via the raw buffer, so a version
    // mismatch between xcap's `image` and ours can never cause a type error.
    let (w, h) = (shot.width(), shot.height());
    RgbaImage::from_raw(w, h, shot.into_raw())
        .ok_or_else(|| anyhow::anyhow!("captured buffer did not match {w}x{h}"))
}

#[cfg(not(feature = "capture"))]
pub fn capture_window_image() -> Result<RgbaImage> {
    anyhow::bail!("screen capture is disabled; build tethys-scanner with the `capture` feature")
}

/// List the title (and minimized state) of every enumerable top-level window.
///
/// A diagnostic for when [`capture_window_image`] can't find the game: run it
/// to see the exact title Wuthering Waves reports, then confirm it matches
/// [`GAME_WINDOW_TITLE`]. Requires the `capture` feature, and must run elevated
/// to see the (elevated) game window.
#[cfg(feature = "capture")]
pub fn list_windows() -> Result<Vec<(String, bool)>> {
    use xcap::Window;

    let windows = Window::all().map_err(|e| anyhow::anyhow!("enumerating windows: {e}"))?;
    Ok(windows
        .into_iter()
        .map(|w| (w.title().to_string(), w.is_minimized()))
        .collect())
}

#[cfg(not(feature = "capture"))]
pub fn list_windows() -> Result<Vec<(String, bool)>> {
    anyhow::bail!("screen capture is disabled; build tethys-scanner with the `capture` feature")
}