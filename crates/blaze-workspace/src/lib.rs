//! Data models for Blaze's first-party productivity and communication panels.
//! These models are local-first foundations; service integrations are separate.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodeDocument {
    pub name: String,
    pub language: String,
    pub source: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Document {
    pub title: String,
    /// Markdown source, intentionally kept editable and portable.
    pub markdown: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Note {
    pub title: String,
    pub body: String,
    pub updated_unix_seconds: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Spreadsheet {
    pub title: String,
    pub rows: Vec<Vec<String>>,
}

impl Spreadsheet {
    /// Exports the grid as RFC-4180-style CSV (UTF-8, CRLF row endings).
    pub fn to_csv(&self) -> String {
        self.rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| csv_escape(cell))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect::<Vec<_>>()
            .join("\r\n")
    }
}

fn csv_escape(cell: &str) -> String {
    let needs_quotes = cell.contains(',')
        || cell.contains('"')
        || cell.contains('\n')
        || cell.contains('\r');
    if needs_quotes {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_owned()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Slide {
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Presentation {
    pub title: String,
    pub slides: Vec<Slide>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageDraft {
    pub recipient: String,
    pub body: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CallState {
    Idle,
    RequestingPermission,
    Connecting,
    Active,
    Ended,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionManifest {
    pub name: String,
    pub version: String,
    pub wasm_entry: String,
    pub requested_capabilities: Vec<ExtensionCapability>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExtensionCapability {
    ReadCurrentPage,
    WriteCurrentPage,
    LocalStorage,
    ClipboardRead,
    ClipboardWrite,
    NetworkAccess,
}

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub fn encode_json<T: Serialize>(value: &T) -> Result<Vec<u8>, ModelError> {
    Ok(serde_json::to_vec_pretty(value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quotes_cells_that_need_escaping() {
        let sheet = Spreadsheet {
            title: "Test".into(),
            rows: vec![
                vec!["name".into(), "note".into()],
                vec!["Blaze".into(), "fast, \"safe\"".into()],
            ],
        };
        assert_eq!(
            sheet.to_csv(),
            "name,note\r\nBlaze,\"fast, \"\"safe\"\"\""
        );
    }

    #[test]
    fn documents_round_trip_as_json() {
        let document = Document {
            title: "Draft".into(),
            markdown: "# Hello".into(),
        };
        let bytes = encode_json(&document).unwrap();
        let decoded: Document = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, document);
    }
}
