//! OCR backends.
//!
//! `WindowsOcr` (behind `windows-ocr`) uses the OS's built-in
//! `Windows.Media.Ocr` engine — no external install, good with the game font.
//! `TesseractOcr` (behind `tesseract`) is a portable alternative that needs a
//! system Tesseract. Both implement the crate's [`OcrEngine`] trait.

#[cfg(feature = "tesseract")]
pub use tesseract_backend::TesseractOcr;

#[cfg(all(windows, feature = "windows-ocr"))]
pub use windows_backend::WindowsOcr;

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

#[cfg(all(windows, feature = "windows-ocr"))]
mod windows_backend {
    use crate::OcrEngine;
    use anyhow::{anyhow, Result};
    use image::{ColorType, ImageEncoder, RgbaImage};
    use std::io::Cursor;
    use windows::core::HSTRING;
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::{BitmapDecoder, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine as WinOcrEngine;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

    /// OCR backend using the built-in `Windows.Media.Ocr` engine.
    ///
    /// Prefers an English recognizer (the game's UI text); falls back to the
    /// user's profile languages. If neither is available, construction fails
    /// with a hint to install the English OCR language feature.
    pub struct WindowsOcr {
        engine: WinOcrEngine,
    }

    impl WindowsOcr {
        pub fn new() -> Result<Self> {
            let english = Language::CreateLanguage(&HSTRING::from("en-US"))?;
            let engine = if WinOcrEngine::IsLanguageSupported(&english)? {
                WinOcrEngine::TryCreateFromLanguage(&english)?
            } else {
                WinOcrEngine::TryCreateFromUserProfileLanguages()?
            };
            Ok(Self { engine })
        }

        /// Encode the crop to PNG in memory, then decode it into a WinRT
        /// `SoftwareBitmap`. The round-trip avoids manual pixel-format and
        /// stride handling and lets the OS decoder produce the Bgra8 the OCR
        /// engine expects.
        fn to_software_bitmap(image: &RgbaImage) -> Result<SoftwareBitmap> {
            let mut png: Vec<u8> = Vec::new();
            image::codecs::png::PngEncoder::new(&mut png).write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                ColorType::Rgba8,
            )?;

            let stream = InMemoryRandomAccessStream::new()?;
            let writer = DataWriter::CreateDataWriter(&stream.GetOutputStreamAt(0)?)?;
            writer.WriteBytes(&png)?;
            writer.StoreAsync()?.get()?;
            writer.FlushAsync()?.get()?;
            stream.Seek(0)?;

            let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
            Ok(decoder.GetSoftwareBitmapAsync()?.get()?)
        }
    }

    impl OcrEngine for WindowsOcr {
        fn recognize(&self, image: &RgbaImage) -> Result<Vec<String>> {
            if image.width() == 0 || image.height() == 0 {
                return Ok(Vec::new());
            }
            let bitmap = Self::to_software_bitmap(image)?;
            let result = self
                .engine
                .RecognizeAsync(&bitmap)
                .map_err(|e| anyhow!("Windows OCR failed: {e}"))?
                .get()?;

            let mut lines = Vec::new();
            for line in result.Lines()? {
                lines.push(line.Text()?.to_string());
            }
            Ok(lines)
        }
    }
}