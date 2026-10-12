use std::{
    fs::File,
    io::{Read, Seek},
    path::Path,
};

use quick_xml::{events::Event, Reader, XmlVersion};
use zip::ZipArchive;

use super::{extracted_text, extraction_failed, ExtractionResult};
use crate::app_error::AppError;

const MAX_XML_BYTES: u64 = 80 * 1024 * 1024;

pub fn extract_office(path: &Path) -> Result<ExtractionResult, AppError> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(extraction_failed)?;
    let file = File::open(path).map_err(|_| extraction_failed())?;
    let mut archive = ZipArchive::new(file).map_err(|_| extraction_failed())?;
    let text = match extension.as_str() {
        "docx" => extract_docx(&mut archive)?,
        "pptx" => extract_pptx(&mut archive)?,
        "xlsx" => extract_xlsx(&mut archive)?,
        _ => return Err(extraction_failed()),
    };
    extracted_text(text)
}

fn extract_docx<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<String, AppError> {
    let xml = read_entry(archive, "word/document.xml")?;
    grouped_text(&xml, b"t", b"p")
}

fn extract_pptx<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<String, AppError> {
    let mut slides = (0..archive.len())
        .filter_map(|index| archive.name_for_index(index).map(str::to_owned))
        .filter_map(|name| {
            let number = name
                .strip_prefix("ppt/slides/slide")?
                .strip_suffix(".xml")?
                .parse::<usize>()
                .ok()?;
            Some((number, name))
        })
        .collect::<Vec<_>>();
    slides.sort_by_key(|(number, _)| *number);

    let mut sections = Vec::new();
    for (position, (_, name)) in slides.into_iter().enumerate() {
        let xml = read_entry(archive, &name)?;
        let body = all_text(&xml, b"t")?;
        sections.push(format!("## 幻灯片 {}\n{}", position + 1, body));
    }
    Ok(sections.join("\n\n"))
}

fn extract_xlsx<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<String, AppError> {
    let workbook = read_entry(archive, "xl/workbook.xml")?;
    let sheet_names = sheet_names(&workbook)?;
    let shared_strings = match read_optional_entry(archive, "xl/sharedStrings.xml")? {
        Some(xml) => grouped_values(&xml, b"t", b"si")?,
        None => Vec::new(),
    };

    let mut sections = Vec::new();
    for (index, name) in sheet_names.into_iter().enumerate() {
        let path = format!("xl/worksheets/sheet{}.xml", index + 1);
        let xml = read_entry(archive, &path)?;
        let rows = worksheet_rows(&xml, &shared_strings)?;
        sections.push(format!("## 工作表：{name}\n{rows}"));
    }
    Ok(sections.join("\n\n"))
}

fn read_entry<R: Read + Seek>(archive: &mut ZipArchive<R>, name: &str) -> Result<String, AppError> {
    read_optional_entry(archive, name)?.ok_or_else(extraction_failed)
}

fn read_optional_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Option<String>, AppError> {
    let Ok(entry) = archive.by_name(name) else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    entry
        .take(MAX_XML_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| extraction_failed())?;
    if bytes.len() as u64 > MAX_XML_BYTES {
        return Err(extraction_failed());
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| extraction_failed())
}

fn all_text(xml: &str, text_tag: &[u8]) -> Result<String, AppError> {
    Ok(text_values(xml, text_tag)?.join("\n"))
}

fn grouped_text(xml: &str, text_tag: &[u8], group_tag: &[u8]) -> Result<String, AppError> {
    Ok(grouped_values(xml, text_tag, group_tag)?.join("\n"))
}

fn grouped_values(xml: &str, text_tag: &[u8], group_tag: &[u8]) -> Result<Vec<String>, AppError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut groups = Vec::new();
    let mut current = String::new();
    let mut capture = false;
    loop {
        match reader.read_event().map_err(|_| extraction_failed())? {
            Event::Start(element) if element.local_name().as_ref() == text_tag => capture = true,
            Event::Text(text) if capture => current.push_str(&decode_text(&text)?),
            Event::End(element) if element.local_name().as_ref() == text_tag => capture = false,
            Event::End(element) if element.local_name().as_ref() == group_tag => {
                if !current.is_empty() {
                    groups.push(std::mem::take(&mut current));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(groups)
}

fn text_values(xml: &str, text_tag: &[u8]) -> Result<Vec<String>, AppError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut values = Vec::new();
    let mut capture = false;
    loop {
        match reader.read_event().map_err(|_| extraction_failed())? {
            Event::Start(element) if element.local_name().as_ref() == text_tag => capture = true,
            Event::Text(text) if capture => values.push(decode_text(&text)?),
            Event::End(element) if element.local_name().as_ref() == text_tag => capture = false,
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(values)
}

fn sheet_names(xml: &str) -> Result<Vec<String>, AppError> {
    let mut reader = Reader::from_str(xml);
    let mut names = Vec::new();
    loop {
        match reader.read_event().map_err(|_| extraction_failed())? {
            Event::Start(element) | Event::Empty(element)
                if element.local_name().as_ref() == b"sheet" =>
            {
                let name = element
                    .attributes()
                    .filter_map(Result::ok)
                    .find(|attribute| attribute.key.local_name().as_ref() == b"name")
                    .ok_or_else(extraction_failed)?
                    .normalized_value(XmlVersion::Implicit1_0)
                    .map_err(|_| extraction_failed())?
                    .into_owned();
                names.push(name);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(names)
}

fn worksheet_rows(xml: &str, shared_strings: &[String]) -> Result<String, AppError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut rows = Vec::new();
    let mut cells = Vec::new();
    let mut cell_type = String::new();
    let mut value = String::new();
    let mut capture = false;
    loop {
        match reader.read_event().map_err(|_| extraction_failed())? {
            Event::Start(element) if element.local_name().as_ref() == b"c" => {
                cell_type = element
                    .attributes()
                    .filter_map(Result::ok)
                    .find(|attribute| attribute.key.local_name().as_ref() == b"t")
                    .and_then(|attribute| attribute.normalized_value(XmlVersion::Implicit1_0).ok())
                    .map(|value| value.into_owned())
                    .unwrap_or_default();
                value.clear();
            }
            Event::Start(element) if matches!(element.local_name().as_ref(), b"v" | b"t") => {
                capture = true;
            }
            Event::Text(text) if capture => value.push_str(&decode_text(&text)?),
            Event::End(element) if matches!(element.local_name().as_ref(), b"v" | b"t") => {
                capture = false;
            }
            Event::End(element) if element.local_name().as_ref() == b"c" => {
                let resolved = if cell_type == "s" {
                    value
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| shared_strings.get(index))
                        .cloned()
                        .unwrap_or_default()
                } else {
                    value.clone()
                };
                cells.push(resolved);
            }
            Event::End(element) if element.local_name().as_ref() == b"row" => {
                if cells.iter().any(|cell| !cell.is_empty()) {
                    rows.push(cells.join("\t"));
                }
                cells.clear();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(rows.join("\n"))
}

fn decode_text(text: &quick_xml::events::BytesText<'_>) -> Result<String, AppError> {
    let decoded = text.decode().map_err(|_| extraction_failed())?;
    quick_xml::escape::unescape(&decoded)
        .map(|value| value.into_owned())
        .map_err(|_| extraction_failed())
}
