//! Renders a synthetic 1920x1080 "game frame" — a left-side echo grid plus a
//! right-side detail panel — and saves the combined calibration overlay, so the
//! region + grid layout can be inspected without a live game.
//! Run: cargo run -p tethys-scanner --example calibration_demo

use image::{Rgba, RgbaImage};
use tethys_scanner::{calibrate, draw_grid_overlay, fit_16_9, EchoDetailLayout, GridLayout};

fn fill_rect(img: &mut RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32, color: [u8; 4]) {
    for y in y0..y1.min(img.height()) {
        for x in x0..x1.min(img.width()) {
            img.put_pixel(x, y, Rgba(color));
        }
    }
}

fn frac(v: f32, whole: u32) -> u32 {
    (v * whole as f32) as u32
}

fn main() {
    let (w, h) = (1920u32, 1080u32);
    let mut frame = RgbaImage::from_pixel(w, h, Rgba([28, 30, 38, 255]));

    // Mock inventory grid tiles on the left (visual reference only).
    let grid = GridLayout::default_16_9();
    let content = fit_16_9(w, h);
    for cell in grid.cells(content) {
        fill_rect(
            &mut frame,
            cell.x,
            cell.y,
            cell.right(),
            cell.bottom(),
            [44, 48, 60, 255],
        );
    }

    // Mock echo detail panel on the right third.
    fill_rect(
        &mut frame,
        frac(0.63, w),
        frac(0.05, h),
        frac(0.99, w),
        frac(0.95, h),
        [46, 50, 64, 255],
    );
    fill_rect(
        &mut frame,
        frac(0.66, w),
        frac(0.085, h),
        frac(0.96, w),
        frac(0.145, h),
        [70, 76, 96, 255],
    );
    fill_rect(
        &mut frame,
        frac(0.66, w),
        frac(0.30, h),
        frac(0.96, w),
        frac(0.36, h),
        [70, 76, 96, 255],
    );

    // Overlay panel regions, then grid cells.
    let overlay = calibrate(&frame, &EchoDetailLayout::default_16_9());
    let overlay = draw_grid_overlay(&overlay, &grid.cells(content));

    let out = "tethys-calibration-demo.png";
    overlay.save(out).unwrap();
    println!("wrote {out}");
}
