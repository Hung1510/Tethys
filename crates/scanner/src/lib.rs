//! Reading Wuthering Waves echoes off the screen.
//!
//! Pipeline: [`capture`] a region of the game window → run an [`OcrEngine`] to
//! get text lines → hand the lines to `tethys_core::parse` to get typed stats.
//!
//! The OCR step is abstracted behind the [`OcrEngine`] trait so the pipeline
//! can be exercised with [`MockOcr`] in tests and with a real backend
//! (Tesseract today, the Windows.Media.Ocr API as a future feature) in
//! production. The parsing that turns text into stats lives in `tethys-core`
//! and is fully tested there.

pub mod capture;
pub mod layout;
pub mod ocr;

use anyhow::Result;
use image::RgbaImage;
use tethys_core::model::StatRoll;
use tethys_core::parse::parse_lines;

pub use layout::{
    crop, draw_grid_overlay, draw_overlay, fit_16_9, EchoDetailLayout, GridLayout, NormRect,
    PixelRect, ResolvedRegions,
};

/// Anything that can turn an image region into lines of text.
pub trait OcrEngine {
    fn recognize(&self, image: &RgbaImage) -> Result<Vec<String>>;
}

/// Run OCR on an image and parse the result into stats. This is the seam
/// between the (platform-specific, hard-to-test) OCR and the (pure, tested)
/// parsing in core.
pub fn read_stats(engine: &dyn OcrEngine, image: &RgbaImage) -> Result<Vec<StatRoll>> {
    let lines = engine.recognize(image)?;
    let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    Ok(parse_lines(refs))
}

/// Locate the echo detail panel inside a captured window image, OCR the main
/// stat and substat regions, and parse them into stats.
///
/// This is the payoff of the layout module: rather than OCR-ing the whole
/// screen, it crops to just the stat regions (computed relative to the 16:9
/// content area), which is both faster and markedly more accurate. Pure given
/// an image and an engine, so it is unit-tested with a synthetic image.
pub fn scan_echo_panel(
    engine: &dyn OcrEngine,
    window_image: &RgbaImage,
    layout: &EchoDetailLayout,
) -> Result<Vec<StatRoll>> {
    let content = fit_16_9(window_image.width(), window_image.height());
    let regions = layout.resolve(content);

    let mut lines: Vec<String> = Vec::new();
    for rect in [regions.main_stat, regions.substats] {
        let region_img = crop(window_image, rect);
        lines.extend(engine.recognize(&region_img)?);
    }

    let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    Ok(parse_lines(refs))
}

/// Produce a calibration overlay: the captured window image with each detected
/// region outlined. Save it and eyeball whether the boxes sit on the real UI,
/// then tune [`EchoDetailLayout`]. Pure and unit-tested.
pub fn calibrate(window_image: &RgbaImage, layout: &EchoDetailLayout) -> RgbaImage {
    let content = fit_16_9(window_image.width(), window_image.height());
    let regions = layout.resolve(content);
    draw_overlay(window_image, &regions.labeled())
}

/// Batch-scan an inventory page: for each tile in the grid, crop it and OCR
/// whatever text the tile shows (main stat, level), returning one parsed stat
/// list per tile in row-major order.
///
/// This is the grid counterpart to [`scan_echo_panel`]. It reuses the same
/// content-relative geometry, so it inherits the letterbox/pillarbox handling
/// for free. Note that inventory tiles show a tile summary, not the full
/// substat list — reading all substats for an echo still uses the detail-panel
/// scan. Tethys reads the screen only and never navigates the game for you.
pub fn scan_grid_tiles(
    engine: &dyn OcrEngine,
    window_image: &RgbaImage,
    grid: &GridLayout,
) -> Result<Vec<Vec<StatRoll>>> {
    let content = fit_16_9(window_image.width(), window_image.height());
    let cells = grid.cells(content);

    let mut tiles = Vec::with_capacity(cells.len());
    for cell in cells {
        let tile_img = crop(window_image, cell);
        let lines = engine.recognize(&tile_img)?;
        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        tiles.push(parse_lines(refs));
    }
    Ok(tiles)
}

/// Produce a calibration overlay for the inventory grid: every cell outlined.
/// Pure and unit-tested.
pub fn calibrate_grid(window_image: &RgbaImage, grid: &GridLayout) -> RgbaImage {
    let content = fit_16_9(window_image.width(), window_image.height());
    let cells = grid.cells(content);
    draw_grid_overlay(window_image, &cells)
}

/// Capture the live game window and run [`scan_echo_panel`] on it.
/// Requires the `capture` feature.
#[cfg(feature = "capture")]
pub fn scan_live(engine: &dyn OcrEngine, layout: &EchoDetailLayout) -> Result<Vec<StatRoll>> {
    let image = capture::capture_window_image()?;
    scan_echo_panel(engine, &image, layout)
}

/// Capture the live game window, draw the calibration overlay(s), and save to
/// `path` as a PNG. Draws the detail-panel regions always, and the inventory
/// grid too when `grid` is `Some`. Requires the `capture` feature.
#[cfg(feature = "capture")]
pub fn save_calibration(
    path: &str,
    panel: &EchoDetailLayout,
    grid: Option<&GridLayout>,
) -> Result<()> {
    let image = capture::capture_window_image()?;
    let mut overlay = calibrate(&image, panel);
    if let Some(grid) = grid {
        let content = fit_16_9(overlay.width(), overlay.height());
        overlay = draw_grid_overlay(&overlay, &grid.cells(content));
    }
    overlay.save(path)?;
    Ok(())
}

/// Without the `capture` feature, saving a calibration overlay fails loudly.
#[cfg(not(feature = "capture"))]
pub fn save_calibration(
    _path: &str,
    _panel: &EchoDetailLayout,
    _grid: Option<&GridLayout>,
) -> Result<()> {
    anyhow::bail!("calibration needs screen capture; build with the `capture` feature")
}

/// A stand-in OCR engine that returns pre-supplied lines. Lets the whole
/// capture→parse pipeline be tested without a screenshot or a native OCR lib.
pub struct MockOcr {
    pub lines: Vec<String>,
}

impl MockOcr {
    pub fn new<I, S>(lines: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            lines: lines.into_iter().map(Into::into).collect(),
        }
    }
}

impl OcrEngine for MockOcr {
    fn recognize(&self, _image: &RgbaImage) -> Result<Vec<String>> {
        Ok(self.lines.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tethys_core::model::Stat;

    #[test]
    fn pipeline_reads_stats_through_mock_ocr() {
        let img = RgbaImage::new(1, 1);
        let engine = MockOcr::new(["Sinking Eclipse", "Crit. Rate 10.5%", "ATK 9.4%", "ATK 40"]);
        let stats = read_stats(&engine, &img).unwrap();
        assert_eq!(stats.len(), 3);
        assert_eq!(stats[0].stat, Stat::CritRate);
        assert_eq!(stats[1].stat, Stat::AtkPct);
        assert_eq!(stats[2].stat, Stat::Atk);
    }

    #[test]
    fn scan_echo_panel_parses_across_window_shapes() {
        // The mock returns the same lines regardless of the crop, so both the
        // main-stat and substats regions contribute them. What matters is that
        // scanning succeeds and parses correctly on differently-shaped windows
        // (exact 16:9, pillarboxed ultrawide, letterboxed 16:10).
        let layout = EchoDetailLayout::default_16_9();
        let engine = MockOcr::new(["Crit. Rate 10.5%", "ATK 40", "Energy Regen 10.0%"]);
        for (w, h) in [(1920, 1080), (2560, 1080), (1920, 1200)] {
            let img = RgbaImage::new(w, h);
            let stats = scan_echo_panel(&engine, &img, &layout).unwrap();
            // 3 stat lines × 2 regions.
            assert_eq!(stats.len(), 6, "wrong count at {w}x{h}");
            assert!(stats.iter().any(|s| s.stat == Stat::CritRate));
            assert!(stats.iter().any(|s| s.stat == Stat::EnergyRegen));
        }
    }

    #[test]
    fn calibrate_returns_annotated_image() {
        let img = RgbaImage::new(1920, 1080);
        let overlay = calibrate(&img, &EchoDetailLayout::default_16_9());
        assert_eq!(overlay.dimensions(), img.dimensions());
        // At least one pixel must differ from the blank input (a drawn border).
        let changed = overlay.pixels().zip(img.pixels()).any(|(a, b)| a != b);
        assert!(changed, "overlay drew nothing");
    }

    #[test]
    fn scan_grid_tiles_returns_one_result_per_cell() {
        let grid = GridLayout::default_16_9();
        let engine = MockOcr::new(["ATK 33.0%", "Lv. 25"]);
        let img = RgbaImage::new(1920, 1080);
        let tiles = scan_grid_tiles(&engine, &img, &grid).unwrap();
        assert_eq!(tiles.len(), grid.cell_count());
        // Each tile parsed the one recognizable stat line (ATK%).
        assert!(tiles
            .iter()
            .all(|t| t.iter().any(|s| s.stat == Stat::AtkPct)));
    }

    #[test]
    fn calibrate_grid_draws_cells() {
        let img = RgbaImage::new(1920, 1080);
        let overlay = calibrate_grid(&img, &GridLayout::default_16_9());
        let changed = overlay.pixels().zip(img.pixels()).any(|(a, b)| a != b);
        assert!(changed, "grid overlay drew nothing");
    }
}
