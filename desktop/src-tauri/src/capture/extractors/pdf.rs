use std::path::Path;

use super::{extracted_text, extraction_failed, ExtractionResult};
use crate::app_error::AppError;

pub fn extract_pdf(path: &Path) -> Result<ExtractionResult, AppError> {
    let text = pdf_extract::extract_text(path).map_err(|_| extraction_failed())?;
    extracted_text(text)
}
