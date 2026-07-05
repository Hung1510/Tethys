//! Locating Wuthering Waves UI regions inside a captured image.
//!
//! # Why this exists
//! Wuthering Waves renders its interface at a 16:9 aspect ratio and adds
//! letterbox (top/bottom) or pillarbox (left/right) bars when the window is any
//! other shape — ultrawide monitors, 16:10 laptops, arbitrary windowed sizes.
//! Hardcoded pixel coordinates therefore break constantly.
//!
//! The robust approach, implemented here, is:
//!   1. Find the 16:9 *content* rectangle inside the captured image
//!      ([`fit_16_9`]), skipping any bars.
//!   2. Express each UI region as a fraction of that content rect ([`NormRect`]).
//!   3. Resolve fractions to absolute pixels ([`EchoDetailLayout::resolve`]).
//!
//! Everything in this module is pure (no capture, no OS calls) and unit-tested,
//! so region maths can be verified without a screenshot or a live game.
//!
//! The default fractions in [`EchoDetailLayout::default_16_9`] are calibration
//! *starting points*. Use the overlay from [`crate::calibrate`] to check them
//! against a real screenshot and adjust — that is the intended workflow.

use image::{Rgba, RgbaImage};

/// Wuthering Waves' native UI aspect ratio.
const TARGET_ASPECT: f64 = 16.0 / 9.0;

/// A rectangle in absolute pixels within a captured image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The exclusive right edge (`x + width`).
    pub fn right(&self) -> u32 {
        self.x + self.width
    }

    /// The exclusive bottom edge (`y + height`).
    pub fn bottom(&self) -> u32 {
        self.y + self.height
    }
}

/// A rectangle expressed as fractions in `[0, 1]` of a 16:9 content area.
#[derive(Debug, Clone, Copy)]
pub struct NormRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl NormRect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// Resolve to absolute pixels within `content` (typically the output of
    /// [`fit_16_9`]).
    pub fn to_pixels(&self, content: PixelRect) -> PixelRect {
        let cw = content.width as f32;
        let ch = content.height as f32;
        PixelRect {
            x: content.x + (self.x.clamp(0.0, 1.0) * cw).round() as u32,
            y: content.y + (self.y.clamp(0.0, 1.0) * ch).round() as u32,
            width: (self.w.clamp(0.0, 1.0) * cw).round() as u32,
            height: (self.h.clamp(0.0, 1.0) * ch).round() as u32,
        }
    }
}

/// Compute the centered 16:9 content rectangle inside an image of the given
/// size, excluding any letterbox/pillarbox bars.
///
/// * Image wider than 16:9 → pillarbox: full height, bars left and right.
/// * Image taller than 16:9 → letterbox: full width, bars top and bottom.
/// * Exactly 16:9 → the whole image.
pub fn fit_16_9(width: u32, height: u32) -> PixelRect {
    if width == 0 || height == 0 {
        return PixelRect::new(0, 0, width, height);
    }
    let w = width as f64;
    let h = height as f64;
    let aspect = w / h;

    if aspect > TARGET_ASPECT {
        // Pillarbox: constrain width to the full height.
        let content_w = (h * TARGET_ASPECT).round();
        let x = ((w - content_w) / 2.0).round();
        PixelRect::new(x as u32, 0, content_w as u32, height)
    } else {
        // Letterbox (or exact): constrain height to the full width.
        let content_h = (w / TARGET_ASPECT).round();
        let y = ((h - content_h) / 2.0).round();
        PixelRect::new(0, y as u32, width, content_h as u32)
    }
}

/// The named regions of the echo detail panel, as fractions of 16:9 content.
///
/// These are the panel that appears when you select an echo. `substats` is the
/// whole block of up to five substat lines; OCR runs on that crop.
#[derive(Debug, Clone, Copy)]
pub struct EchoDetailLayout {
    pub name: NormRect,
    pub cost: NormRect,
    pub main_stat: NormRect,
    pub substats: NormRect,
    pub set: NormRect,
}

impl EchoDetailLayout {
    /// Calibration starting points for the 16:9 layout. Verify against a real
    /// screenshot with the [`crate::calibrate`] overlay and tune as needed.
    pub fn default_16_9() -> Self {
        Self {
            name: NormRect::new(0.660, 0.085, 0.300, 0.060),
            cost: NormRect::new(0.905, 0.090, 0.060, 0.050),
            main_stat: NormRect::new(0.660, 0.300, 0.300, 0.060),
            substats: NormRect::new(0.660, 0.545, 0.310, 0.300),
            set: NormRect::new(0.660, 0.885, 0.310, 0.055),
        }
    }

    /// Resolve every region to absolute pixels within `content`.
    pub fn resolve(&self, content: PixelRect) -> ResolvedRegions {
        ResolvedRegions {
            name: self.name.to_pixels(content),
            cost: self.cost.to_pixels(content),
            main_stat: self.main_stat.to_pixels(content),
            substats: self.substats.to_pixels(content),
            set: self.set.to_pixels(content),
        }
    }
}

/// Absolute-pixel versions of [`EchoDetailLayout`].
#[derive(Debug, Clone, Copy)]
pub struct ResolvedRegions {
    pub name: PixelRect,
    pub cost: PixelRect,
    pub main_stat: PixelRect,
    pub substats: PixelRect,
    pub set: PixelRect,
}

impl ResolvedRegions {
    /// The regions paired with human-readable labels, for overlays and logs.
    pub fn labeled(&self) -> [(&'static str, PixelRect); 5] {
        [
            ("name", self.name),
            ("cost", self.cost),
            ("main_stat", self.main_stat),
            ("substats", self.substats),
            ("set", self.set),
        ]
    }
}

/// Crop a sub-image, clamping the rectangle to the image bounds so an
/// over-large or off-screen region can never panic.
pub fn crop(img: &RgbaImage, rect: PixelRect) -> RgbaImage {
    let x = rect.x.min(img.width());
    let y = rect.y.min(img.height());
    let w = rect.width.min(img.width().saturating_sub(x));
    let h = rect.height.min(img.height().saturating_sub(y));
    image::imageops::crop_imm(img, x, y, w, h).to_image()
}

/// Distinct colors used to outline regions in the calibration overlay.
const OVERLAY_COLORS: [[u8; 4]; 5] = [
    [255, 64, 64, 255],   // name    — red
    [255, 214, 64, 255],  // cost    — amber
    [64, 220, 120, 255],  // main    — green
    [64, 200, 255, 255],  // substats — cyan
    [200, 120, 255, 255], // set     — purple
];

/// Return a copy of `img` with each region drawn as a colored rectangle. Used
/// to visually verify that the layout lines up with the real UI.
pub fn draw_overlay(img: &RgbaImage, regions: &[(&str, PixelRect)]) -> RgbaImage {
    let mut out = img.clone();
    for (i, (_, rect)) in regions.iter().enumerate() {
        draw_border(&mut out, *rect, OVERLAY_COLORS[i % OVERLAY_COLORS.len()], 3);
    }
    out
}

/// Draw a `thickness`-pixel border of `color` around `rect`, clipped to the
/// image.
fn draw_border(img: &mut RgbaImage, rect: PixelRect, color: [u8; 4], thickness: u32) {
    let (iw, ih) = (img.width(), img.height());
    let x0 = rect.x.min(iw);
    let y0 = rect.y.min(ih);
    let x1 = rect.right().min(iw);
    let y1 = rect.bottom().min(ih);
    let px = Rgba(color);

    // Top and bottom edges.
    for x in x0..x1 {
        for d in 0..thickness {
            let top = y0 + d;
            if top < ih {
                img.put_pixel(x, top, px);
            }
            if y1 > d {
                let bottom = y1 - 1 - d;
                if bottom < ih {
                    img.put_pixel(x, bottom, px);
                }
            }
        }
    }
    // Left and right edges.
    for y in y0..y1 {
        for d in 0..thickness {
            let left = x0 + d;
            if left < iw {
                img.put_pixel(left, y, px);
            }
            if x1 > d {
                let right = x1 - 1 - d;
                if right < iw {
                    img.put_pixel(right, y, px);
                }
            }
        }
    }
}

/// The echo *inventory grid*: a tiled area of echo icons on the inventory page.
///
/// This reuses the same content-relative approach as [`EchoDetailLayout`]: the
/// grid's bounding box is a fraction of the 16:9 content area, and cells are
/// derived from a column/row count plus inter-cell gaps. Computing every cell
/// rectangle is what lets a whole page be batch-scanned in one pass instead of
/// one echo at a time.
#[derive(Debug, Clone, Copy)]
pub struct GridLayout {
    /// Bounding box of the tile grid, as a fraction of 16:9 content.
    pub area: NormRect,
    pub cols: u32,
    pub rows: u32,
    /// Gap between columns, as a fraction of the grid area's width.
    pub col_gap: f32,
    /// Gap between rows, as a fraction of the grid area's height.
    pub row_gap: f32,
}

impl GridLayout {
    /// Calibration starting point for a typical 16:9 inventory page: five
    /// columns of tiles down the left side. Verify with the grid overlay and
    /// tune, exactly as with the detail-panel layout.
    /// Calibration starting point for a typical 16:9 inventory page. The echo
    /// backpack shows a 4-column by 6-row grid (24 tiles) per page — a data
    /// point taken from the community's WuWaOpt scanner. Verify with the grid
    /// overlay and tune as needed.
    pub fn default_16_9() -> Self {
        Self {
            area: NormRect::new(0.055, 0.175, 0.560, 0.760),
            cols: 4,
            rows: 6,
            col_gap: 0.020,
            row_gap: 0.022,
        }
    }

    /// The number of tiles this layout describes (`cols * rows`).
    pub fn cell_count(&self) -> usize {
        (self.cols * self.rows) as usize
    }

    /// Compute the pixel rectangle of every cell, in row-major order
    /// (left-to-right, top-to-bottom), within `content`.
    pub fn cells(&self, content: PixelRect) -> Vec<PixelRect> {
        let area = self.area.to_pixels(content);
        let mut out = Vec::with_capacity(self.cell_count());
        if self.cols == 0 || self.rows == 0 {
            return out;
        }

        let aw = area.width as f32;
        let ah = area.height as f32;
        // Gap totals leave the remainder for cells.
        let cell_w = (aw * (1.0 - self.col_gap * (self.cols.saturating_sub(1)) as f32)
            / self.cols as f32)
            .max(0.0);
        let cell_h = (ah * (1.0 - self.row_gap * (self.rows.saturating_sub(1)) as f32)
            / self.rows as f32)
            .max(0.0);
        let step_x = cell_w + aw * self.col_gap;
        let step_y = cell_h + ah * self.row_gap;

        for r in 0..self.rows {
            for c in 0..self.cols {
                out.push(PixelRect {
                    x: area.x + (c as f32 * step_x).round() as u32,
                    y: area.y + (r as f32 * step_y).round() as u32,
                    width: cell_w.round() as u32,
                    height: cell_h.round() as u32,
                });
            }
        }
        out
    }

    /// The center point of every cell — useful for a future navigation helper
    /// (Tethys never injects input itself; see the project's no-automation
    /// stance).
    pub fn cell_centers(&self, content: PixelRect) -> Vec<(u32, u32)> {
        self.cells(content)
            .into_iter()
            .map(|r| (r.x + r.width / 2, r.y + r.height / 2))
            .collect()
    }
}

/// Color used to outline grid cells in the calibration overlay (magenta).
const GRID_COLOR: [u8; 4] = [230, 90, 230, 255];

/// Return a copy of `img` with every grid cell outlined. Use it to check the
/// grid lines up with the real inventory tiles before batch-scanning.
pub fn draw_grid_overlay(img: &RgbaImage, cells: &[PixelRect]) -> RgbaImage {
    let mut out = img.clone();
    for cell in cells {
        draw_border(&mut out, *cell, GRID_COLOR, 2);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_exact_16_9_is_full_image() {
        assert_eq!(fit_16_9(1920, 1080), PixelRect::new(0, 0, 1920, 1080));
    }

    #[test]
    fn fit_ultrawide_pillarboxes() {
        // 21:9-ish monitor: bars left and right, 16:9 content centered.
        let c = fit_16_9(2560, 1080);
        assert_eq!(c, PixelRect::new(320, 0, 1920, 1080));
    }

    #[test]
    fn fit_16_10_letterboxes() {
        // 16:10 laptop: bars top and bottom.
        let c = fit_16_9(1920, 1200);
        assert_eq!(c, PixelRect::new(0, 60, 1920, 1080));
    }

    #[test]
    fn norm_rect_offsets_into_content() {
        // A region at the content origin must be offset by a pillarboxed
        // content's x, not sit at the image's left edge.
        let content = fit_16_9(2560, 1080); // x = 320
        let r = NormRect::new(0.0, 0.0, 0.5, 0.5).to_pixels(content);
        assert_eq!(r.x, 320);
        assert_eq!(r.y, 0);
        assert_eq!(r.width, 960);
        assert_eq!(r.height, 540);
    }

    #[test]
    fn resolved_regions_stay_inside_content() {
        for (w, h) in [(1920, 1080), (2560, 1080), (1920, 1200), (1366, 768)] {
            let content = fit_16_9(w, h);
            let regions = EchoDetailLayout::default_16_9().resolve(content);
            for (name, rect) in regions.labeled() {
                assert!(
                    rect.x >= content.x && rect.right() <= content.right(),
                    "{name} x out of content at {w}x{h}"
                );
                assert!(
                    rect.y >= content.y && rect.bottom() <= content.bottom(),
                    "{name} y out of content at {w}x{h}"
                );
            }
        }
    }

    #[test]
    fn crop_clamps_oversized_rect() {
        let img = RgbaImage::new(100, 100);
        let cropped = crop(&img, PixelRect::new(80, 80, 50, 50));
        assert_eq!(cropped.dimensions(), (20, 20));
    }

    #[test]
    fn overlay_draws_something() {
        let img = RgbaImage::new(200, 200);
        let regions = [("box", PixelRect::new(10, 10, 50, 50))];
        let out = draw_overlay(&img, &regions);
        assert_eq!(out.dimensions(), img.dimensions());
        // A pixel on the border should now be colored.
        assert_eq!(out.get_pixel(10, 10), &Rgba(OVERLAY_COLORS[0]));
        // The interior stays untouched.
        assert_eq!(out.get_pixel(35, 35), img.get_pixel(35, 35));
    }

    #[test]
    fn grid_produces_all_cells_in_row_major_order() {
        let content = fit_16_9(1920, 1080);
        let grid = GridLayout::default_16_9();
        let cells = grid.cells(content);
        assert_eq!(cells.len(), grid.cell_count());
        assert_eq!(cells.len(), 24); // 4 x 6

        // Row-major: within a row x increases; between rows y increases.
        for r in 0..grid.rows as usize {
            for c in 1..grid.cols as usize {
                let prev = cells[r * grid.cols as usize + c - 1];
                let cur = cells[r * grid.cols as usize + c];
                assert!(cur.x > prev.x, "cells not left-to-right");
                assert!(cur.x >= prev.right(), "cells overlap horizontally");
            }
        }
        for r in 1..grid.rows as usize {
            let above = cells[(r - 1) * grid.cols as usize];
            let below = cells[r * grid.cols as usize];
            assert!(below.y >= above.bottom(), "rows overlap vertically");
        }
    }

    #[test]
    fn grid_cells_stay_within_content() {
        for (w, h) in [(1920, 1080), (2560, 1080), (1920, 1200)] {
            let content = fit_16_9(w, h);
            let cells = GridLayout::default_16_9().cells(content);
            for cell in cells {
                assert!(cell.x >= content.x && cell.right() <= content.right());
                assert!(cell.y >= content.y && cell.bottom() <= content.bottom());
            }
        }
    }

    #[test]
    fn grid_centers_are_inside_their_cells() {
        let content = fit_16_9(1920, 1080);
        let grid = GridLayout::default_16_9();
        let cells = grid.cells(content);
        let centers = grid.cell_centers(content);
        assert_eq!(cells.len(), centers.len());
        for (cell, (cx, cy)) in cells.iter().zip(centers) {
            assert!(cx >= cell.x && cx <= cell.right());
            assert!(cy >= cell.y && cy <= cell.bottom());
        }
    }
}