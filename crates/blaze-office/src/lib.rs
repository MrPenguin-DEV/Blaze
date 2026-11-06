//! Office Open XML import/export bridge for Blaze's Markdown-first editors.
//!
//! office_oxide handles the actual OOXML containers and format conversion.
//! This adapter intentionally reports parse damage to callers instead of
//! silently presenting a partially recovered document as lossless.

use office_oxide::{create::create_from_markdown, format::DocumentFormat, Document};
use std::path::Path;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfficeOutputFormat {
    Docx,
    Xlsx,
    Pptx,
}

impl OfficeOutputFormat {
    pub const fn document_format(self) -> DocumentFormat {
        match self {
            Self::Docx => DocumentFormat::Docx,
            Self::Xlsx => DocumentFormat::Xlsx,
            Self::Pptx => DocumentFormat::Pptx,
        }
    }

    pub const fn extension(self) -> &'static str {
        self.document_format().extension()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfficeImport {
    pub format: DocumentFormat,
    pub markdown: String,
    pub plain_text: String,
    pub damage_count: usize,
}

/// Unified Office import. Detects modern OOXML and legacy binary formats.
/// The caller receives known repair/truncation information in damage_count.
pub fn import_office(path: impl AsRef<Path>) -> Result<OfficeImport, office_oxide::OfficeError> {
    let document = Document::open(path)?;
    let damage_count = document.damage().len();
    Ok(OfficeImport {
        format: document.format(),
        markdown: document.to_markdown(),
        plain_text: document.plain_text(),
        damage_count,
    })
}

/// Writes Markdown into a modern OOXML file. Legacy binary output formats are
/// deliberately not offered. This is an interchange export, not a guarantee
/// of feature-perfect Office round-tripping.
pub fn export_markdown(
    markdown: &str,
    format: OfficeOutputFormat,
    path: impl AsRef<Path>,
) -> Result<(), office_oxide::OfficeError> {
    create_from_markdown(markdown, format.document_format(), path)
}

#[derive(Debug, Error)]
pub enum OfficeBridgeError {
    #[error("Office document path has no extension")]
    MissingExtension,
    #[error("unsupported Office format: {0}")]
    UnsupportedFormat(String),
}

pub fn detect_supported_output(
    path: impl AsRef<Path>,
) -> Result<OfficeOutputFormat, OfficeBridgeError> {
    let extension = path
        .as_ref()
        .extension()
        .and_then(|value| value.to_str())
        .ok_or(OfficeBridgeError::MissingExtension)?
        .to_ascii_lowercase();

    match extension.as_str() {
        "docx" => Ok(OfficeOutputFormat::Docx),
        "xlsx" => Ok(OfficeOutputFormat::Xlsx),
        "pptx" => Ok(OfficeOutputFormat::Pptx),
        other => Err(OfficeBridgeError::UnsupportedFormat(other.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_only_modern_ooxml_output_formats() {
        assert_eq!(detect_supported_output("report.docx").unwrap(), OfficeOutputFormat::Docx);
        assert_eq!(detect_supported_output("table.xlsx").unwrap(), OfficeOutputFormat::Xlsx);
        assert_eq!(detect_supported_output("deck.pptx").unwrap(), OfficeOutputFormat::Pptx);
        assert!(detect_supported_output("legacy.doc").is_err());
    }
}
