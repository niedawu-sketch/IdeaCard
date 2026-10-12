use std::path::Path;

use crate::{app_error::AppError, domain::card::ExtractionStatus};

pub mod office;
pub mod pdf;
pub mod text;

pub const MAX_EXTRACTED_CHARACTERS: usize = 20_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionResult {
    pub status: ExtractionStatus,
    pub text: String,
    pub error: Option<String>,
}

pub trait TextExtractor {
    fn extract(&self, path: &Path) -> Result<ExtractionResult, AppError>;
}

pub(crate) fn extracted_text(text: String) -> Result<ExtractionResult, AppError> {
    if text.chars().count() > MAX_EXTRACTED_CHARACTERS {
        return Err(AppError::new(
            "file_text_too_large",
            "文件提取后的文字超过 2000 万字符",
            "拆分文件后重新导入",
        ));
    }
    let status = if text.trim().is_empty() {
        ExtractionStatus::Empty
    } else {
        ExtractionStatus::Completed
    };
    Ok(ExtractionResult {
        status,
        text,
        error: None,
    })
}

pub(crate) fn extraction_failed() -> AppError {
    AppError::new(
        "file_extraction_failed",
        "无法提取文件内容",
        "保留原文件并手工补充内容，或换用可读取的文件",
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write, path::Path};

    use tempfile::tempdir;
    use zip::{write::SimpleFileOptions, ZipWriter};

    use super::{office::extract_office, pdf::extract_pdf, text::extract_txt};

    #[test]
    fn extracts_utf8_text_and_pdf_content() {
        let directory = tempdir().unwrap();
        let text_path = directory.path().join("notes.txt");
        let pdf_path = directory.path().join("report.pdf");
        fs::write(&text_path, "第一行\n第二行").unwrap();
        write_minimal_pdf(&pdf_path, "IdeaCard PDF");

        assert_eq!(extract_txt(&text_path).unwrap().text, "第一行\n第二行");
        assert!(extract_pdf(&pdf_path)
            .unwrap()
            .text
            .contains("IdeaCard PDF"));
    }

    #[test]
    fn extracts_docx_pptx_and_xlsx_in_document_order() {
        let directory = tempdir().unwrap();
        let docx = directory.path().join("document.docx");
        let pptx = directory.path().join("slides.pptx");
        let xlsx = directory.path().join("workbook.xlsx");

        write_zip(
            &docx,
            &[(
                "word/document.xml",
                r#"<w:document xmlns:w="w"><w:body><w:p><w:r><w:t>标题</w:t></w:r></w:p><w:p><w:r><w:t>正文</w:t></w:r></w:p></w:body></w:document>"#,
            )],
        );
        write_zip(
            &pptx,
            &[
                (
                    "ppt/slides/slide2.xml",
                    r#"<p:sld xmlns:p="p" xmlns:a="a"><a:t>第二页</a:t></p:sld>"#,
                ),
                (
                    "ppt/slides/slide1.xml",
                    r#"<p:sld xmlns:p="p" xmlns:a="a"><a:t>第一页</a:t></p:sld>"#,
                ),
            ],
        );
        write_zip(
            &xlsx,
            &[
                (
                    "xl/workbook.xml",
                    r#"<workbook xmlns:r="r"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
                ),
                (
                    "xl/sharedStrings.xml",
                    r#"<sst><si><t>表头</t></si><si><t>内容</t></si></sst>"#,
                ),
                (
                    "xl/worksheets/sheet1.xml",
                    r#"<worksheet><sheetData><row><c t="s"><v>0</v></c><c t="s"><v>1</v></c></row></sheetData></worksheet>"#,
                ),
            ],
        );

        assert_eq!(extract_office(&docx).unwrap().text, "标题\n正文");
        let slides = extract_office(&pptx).unwrap().text;
        assert!(slides.starts_with("## 幻灯片 1\n第一页"));
        assert!(slides.contains("## 幻灯片 2\n第二页"));
        let workbook = extract_office(&xlsx).unwrap().text;
        assert!(workbook.contains("## 工作表：Sheet1"));
        assert!(workbook.contains("表头\t内容"));
    }

    #[test]
    fn rejects_corrupt_office_files_with_a_stable_error() {
        let directory = tempdir().unwrap();
        let corrupt = directory.path().join("corrupt.docx");
        fs::write(&corrupt, b"not a zip").unwrap();

        assert_eq!(
            extract_office(&corrupt).unwrap_err().code,
            "file_extraction_failed"
        );
    }

    fn write_zip(path: &Path, entries: &[(&str, &str)]) {
        let file = fs::File::create(path).unwrap();
        let mut archive = ZipWriter::new(file);
        for (name, body) in entries {
            archive
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            archive.write_all(body.as_bytes()).unwrap();
        }
        archive.finish().unwrap();
    }

    fn write_minimal_pdf(path: &Path, text: &str) {
        let stream = format!("BT /F1 18 Tf 50 100 Td ({text}) Tj ET");
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_owned(),
            format!("<< /Length {} >>\nstream\n{}\nendstream", stream.len(), stream),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        ];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
        }
        let xref = pdf.len();
        pdf.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
        );
        for offset in offsets {
            pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        fs::write(path, pdf).unwrap();
    }
}
