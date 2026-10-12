use std::{fs, path::Path};

use encoding_rs::{UTF_16BE, UTF_16LE};

use super::{extracted_text, extraction_failed, ExtractionResult};
use crate::app_error::AppError;

pub fn extract_txt(path: &Path) -> Result<ExtractionResult, AppError> {
    let bytes = fs::read(path).map_err(|_| extraction_failed())?;
    let text = if let Some(bytes) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8(bytes.to_vec()).map_err(|_| extraction_failed())?
    } else if let Some(bytes) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let (text, _, malformed) = UTF_16LE.decode(bytes);
        if malformed {
            return Err(extraction_failed());
        }
        text.into_owned()
    } else if let Some(bytes) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let (text, _, malformed) = UTF_16BE.decode(bytes);
        if malformed {
            return Err(extraction_failed());
        }
        text.into_owned()
    } else {
        String::from_utf8(bytes).map_err(|_| extraction_failed())?
    };

    extracted_text(text)
}
