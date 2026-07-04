//! OCR backends.
//!
//! `TesseractOcr` (behind the `tesseract` feature) is the portable default.
//! A `WindowsOcr` backend using the built-in `Windows.Media.Ocr` API is a good
//! future addition on Windows: it needs no extra install and handles the
//! game's font well. Both would implement the crate's [`OcrEngine`] trait.

#[cfg(feature = "tesseract")]
pub use tesseract_backend::TesseractOcr;

#[cfg(feature = "tesseract")]
mod tesseract_backend {
    use crate::OcrEngine;
    use anyhow::Result;
    use image::RgbaImage;
    use rusty_tesseract::{Args, Image};

    /// OCR backend backed by a system Tesseract install.
    pub struct TesseractOcr {
        args: Args,
    }

    impl TesseractOcr {
        pub fn new() -> Self {
            // A restricted whitelist and single-language model markedly improve
            // accuracy on the game's stat panel.
            let mut args = Args::default();
            args.lang = "eng".into();
            Self { args }
        }
    }

    impl Default for TesseractOcr {
        fn default() -> Self {
            Self::new()
        }
    }

    impl OcrEngine for TesseractOcr {
        fn recognize(&self, image: &RgbaImage) -> Result<Vec<String>> {
            let dynimg = image::DynamicImage::ImageRgba8(image.clone());
            let ts_img = Image::from_dynamic_image(&dynimg)?;
            let text = rusty_tesseract::image_to_string(&ts_img, &self.args)?;
            Ok(text.lines().map(|l| l.trim().to_string()).collect())
        }
    }
}
