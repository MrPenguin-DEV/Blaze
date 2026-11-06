//! Office Open XML import/export bridge for Blaze's editors.
//!
//! Office Oxide handles OOXML containers. Markdown is used as the interchange
//! representation for document and presentation structure; spreadsheets use
//! explicit CSV <-> Markdown-table conversion so rows and cells are preserved.

use office_oxide::{create::create_from_markdown, format::DocumentFormat, Document};
use spreadsheet_formula::{CellRef, Evaluator, Sheet, Value};
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

    pub fn extension(self) -> &'static str {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfficeCsvImport {
    pub format: DocumentFormat,
    pub csv: String,
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

/// Imports an Office spreadsheet through the library's Markdown table
/// representation and converts that table into editable RFC-style CSV.
pub fn import_spreadsheet_as_csv(
    path: impl AsRef<Path>,
) -> Result<OfficeCsvImport, OfficeBridgeError> {
    let document = Document::open(path)?;
    let markdown = document.to_markdown();
    let csv = markdown_table_to_csv(&markdown)
        .ok_or(OfficeBridgeError::NoSpreadsheetTable)?;
    Ok(OfficeCsvImport {
        format: document.format(),
        csv,
        damage_count: document.damage().len(),
    })
}

/// Writes Markdown into a modern OOXML file. This is an interchange export,
/// not a guarantee of feature-perfect Office round-tripping.
pub fn export_markdown(
    markdown: &str,
    format: OfficeOutputFormat,
    path: impl AsRef<Path>,
) -> Result<(), office_oxide::OfficeError> {
    create_from_markdown(markdown, format.document_format(), path)
}

/// Converts the editor's CSV grid to a Markdown table and writes an XLSX file.
/// Formatting, formulas, charts and merged cells need the format-specific API.
pub fn export_csv(
    csv: &str,
    path: impl AsRef<Path>,
) -> Result<(), OfficeBridgeError> {
    let rows = parse_csv(csv)?;
    let markdown = rows_to_markdown_table(&rows);
    create_from_markdown(&markdown, DocumentFormat::Xlsx, path)?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum OfficeBridgeError {
    #[error("Office document path has no extension")]
    MissingExtension,
    #[error("unsupported Office format: {0}")]
    UnsupportedFormat(String),
    #[error("CSV is malformed: {0}")]
    InvalidCsv(String),
    #[error("the imported document did not contain a Markdown table")]
    NoSpreadsheetTable,
    #[error("formula could not be evaluated: {0}")]
    Formula(String),
    #[error("Office conversion failed: {0}")]
    Office(#[from] office_oxide::OfficeError),
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

/// Parses UTF-8 CSV, including quoted commas, doubled quotes and embedded
/// newlines. A malformed or unterminated quoted field is rejected.
pub fn parse_csv(input: &str) -> Result<Vec<Vec<String>>, OfficeBridgeError> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = input.chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        if in_quotes {
            match ch {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => in_quotes = false,
                other => field.push(other),
            }
            continue;
        }

        match ch {
            '"' if field.is_empty() => in_quotes = true,
            '"' => return Err(OfficeBridgeError::InvalidCsv(
                "quote found inside an unquoted field".to_owned(),
            )),
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            other => field.push(other),
        }
    }

    if in_quotes {
        return Err(OfficeBridgeError::InvalidCsv(
            "quoted field was not terminated".to_owned(),
        ));
    }

    if !field.is_empty() || !row.is_empty() || (!input.is_empty() && !(input.ends_with('\n') || input.ends_with('\r'))) {
        row.push(field);
        rows.push(row);
    }

    Ok(rows)
}

/// A single-sheet, read-only adapter over parsed CSV values.
#[derive(Debug)]
struct CsvFormulaSheet {
    rows: Vec<Vec<String>>,
}

impl Sheet for CsvFormulaSheet {
    fn value(&self, cell: CellRef) -> Value {
        self.rows
            .get(cell.row as usize)
            .and_then(|row| row.get(cell.column as usize))
            .map(|value| {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    Value::Empty
                } else if trimmed.eq_ignore_ascii_case("true") {
                    Value::Bool(true)
                } else if trimmed.eq_ignore_ascii_case("false") {
                    Value::Bool(false)
                } else if let Ok(number) = trimmed.parse::<f64>() {
                    Value::Number(number)
                } else {
                    Value::Text(value.clone())
                }
            })
            .unwrap_or(Value::Empty)
    }
}

/// Evaluates an Excel-style formula against a CSV grid.
///
/// Cell coordinates and ranges use ordinary A1 notation. The evaluator supports
/// hundreds of spreadsheet functions and rejects unsupported functions,
/// malformed expressions and circular references. Range materialization is
/// capped to 10,000 cells to bound hostile formulas.
pub fn evaluate_csv_formula(
    csv: &str,
    formula: &str,
) -> Result<String, OfficeBridgeError> {
    if formula.trim().is_empty() {
        return Err(OfficeBridgeError::Formula("formula is empty".to_owned()));
    }

    let sheet = CsvFormulaSheet {
        rows: parse_csv(csv)?,
    };
    let mut evaluator = Evaluator::new(&sheet).with_max_range_cells(10_000);
    let value = evaluator
        .eval(formula)
        .map_err(|error| OfficeBridgeError::Formula(error.to_string()))?;
    Ok(value.to_string())
}

/// Converts CSV rows to a GitHub-Flavored Markdown table.
pub fn csv_to_markdown_table(csv: &str) -> Result<String, OfficeBridgeError> {
    Ok(rows_to_markdown_table(&parse_csv(csv)?))
}

fn rows_to_markdown_table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return "| |\n| --- |\n".to_owned();
    }

    let columns = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let mut output = String::new();

    for (row_index, row) in rows.iter().enumerate() {
        output.push('|');
        for column in 0..columns {
            let value = row.get(column).map(String::as_str).unwrap_or("");
            output.push(' ');
            output.push_str(&escape_markdown_cell(value));
            output.push(' ');
            output.push('|');
        }
        output.push('\n');

        if row_index == 0 {
            output.push('|');
            for _ in 0..columns {
                output.push_str(" --- |");
            }
            output.push('\n');
        }
    }
    output
}

fn escape_markdown_cell(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\r\n', "<br>")
        .replace('\n', "<br>")
        .replace('\r', "<br>")
}

fn markdown_table_to_csv(markdown: &str) -> Option<String> {
    let mut rows = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if !trimmed.contains('|') {
            continue;
        }
        let cells = split_markdown_row(trimmed);
        if cells.is_empty() {
            continue;
        }
        if cells.iter().all(|cell| is_markdown_separator(cell)) {
            continue;
        }
        rows.push(cells);
    }

    if rows.is_empty() {
        return None;
    }

    let columns = rows.iter().map(Vec::len).max().unwrap_or(1);
    let mut output = String::new();
    for (row_index, row) in rows.iter().enumerate() {
        if row_index > 0 {
            output.push_str("\r\n");
        }
        for column in 0..columns {
            if column > 0 {
                output.push(',');
            }
            let cell = row.get(column).map(String::as_str).unwrap_or("");
            output.push_str(&escape_csv_cell(&cell.replace("<br>", "\n")));
        }
    }
    Some(output)
}

fn split_markdown_row(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut chars = line.chars().peekable();
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if escaped {
            cell.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '|' => {
                cells.push(cell.trim().to_owned());
                cell.clear();
            }
            other => cell.push(other),
        }
    }
    if escaped {
        cell.push('\\');
    }
    if !cell.is_empty() || !cells.is_empty() {
        cells.push(cell.trim().to_owned());
    }

    if line.trim_start().starts_with('|') && !cells.is_empty() {
        cells.remove(0);
    }
    if line.trim_end().ends_with('|') && !cells.is_empty() {
        cells.pop();
    }
    cells
}

fn is_markdown_separator(cell: &str) -> bool {
    let trimmed = cell.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|ch| ch == '-' || ch == ':' || ch.is_whitespace())
        && trimmed.contains('-')
}

fn escape_csv_cell(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
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

    #[test]
    fn parses_csv_quotes_and_embedded_newlines() {
        let rows = parse_csv("name,note\r\nBlaze,\"fast, \"\"safe\"\"\"\r\n\"two\nlines\",ok").unwrap();
        assert_eq!(rows[1][1], "fast, \"safe\"");
        assert_eq!(rows[2][0], "two\nlines");
    }

    #[test]
    fn csv_markdown_table_round_trip() {
        let csv = "Name,Note\r\nBlaze,\"fast, \"\"safe\"\"\"\r\nSecond,row";
        let markdown = csv_to_markdown_table(csv).unwrap();
        assert_eq!(
            markdown_table_to_csv(&markdown).unwrap(),
            "Name,Note\r\nBlaze,\"fast, \"\"safe\"\"\"\r\nSecond,row"
        );
    }

    #[test]
    fn evaluates_excel_formulas_against_csv_cells() {
        assert_eq!(
            evaluate_csv_formula("Item,Quantity\nApples,4\nPears,6", "=SUM(B2:B3)*2").unwrap(),
            "20"
        );
        assert_eq!(
            evaluate_csv_formula("Name,Score\nAda,90\nLin,95", "=AVERAGE(B2:B3)").unwrap(),
            "92.5"
        );
    }

    #[test]
    fn formula_range_expansion_is_bounded() {
        let error = evaluate_csv_formula("x\n1", "=SUM(A1:XFD1048576)").unwrap_err();
        assert!(matches!(error, OfficeBridgeError::Formula(_)));
    }

    #[test]
    fn docx_markdown_export_can_be_reopened() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document.docx");
        export_markdown("# Blaze document\n\nA local-first draft.", OfficeOutputFormat::Docx, &path).unwrap();
        let imported = import_office(&path).unwrap();
        assert!(imported.plain_text.contains("Blaze document"));
    }

    #[test]
    fn xlsx_csv_export_can_be_reopened() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sheet.xlsx");
        export_csv("Item,Quantity\nApples,4\nPears,6", &path).unwrap();
        let imported = import_spreadsheet_as_csv(&path).unwrap();
        let rows = parse_csv(&imported.csv).unwrap();
        assert_eq!(rows[0], vec!["Item", "Quantity"]);
        assert_eq!(rows[2], vec!["Pears", "6"]);
    }

    #[test]
    fn pptx_markdown_export_can_be_reopened() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("slides.pptx");
        export_markdown(
            "# Quarterly Plan\n\nShip a reliable browser prototype.",
            OfficeOutputFormat::Pptx,
            &path,
        )
        .unwrap();
        let imported = import_office(&path).unwrap();
        assert!(imported.plain_text.contains("Quarterly Plan"));
    }

    #[test]
    fn rejects_unterminated_csv_quote() {
        assert!(matches!(
            parse_csv("a,\"broken"),
            Err(OfficeBridgeError::InvalidCsv(_))
        ));
    }
}
