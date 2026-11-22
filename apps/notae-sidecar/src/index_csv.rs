//! Reading the `Indeks.csv` catalogue.

use std::collections::HashMap;
use std::path::Path;

use crate::ToolError;

/// The catalogue fields of one `Indeks.csv` row.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CsvRow {
    /// `ID` column.
    pub id: Option<u32>,
    /// `Tytuł` column.
    pub title: Option<String>,
    /// `Autor` column.
    pub author: Option<String>,
    /// `Rodzaj` column.
    pub kind: Option<String>,
    /// `Tagi` column (optional), split on `;`.
    pub tags: Vec<String>,
    /// `Tekst OCR` column.
    pub text: Option<String>,
}

/// Reads the catalogue, keyed by its `Nazwa pliku` column.
///
/// The file is UTF-8, with or without a byte order mark.
///
/// # Errors
/// Returns an error when the file cannot be read, is not valid CSV, lacks the
/// `Nazwa pliku` column, or has an `ID` that is not a number.
pub fn read_index(path: &Path) -> Result<HashMap<String, CsvRow>, ToolError> {
    let ctx = || format!("reading {}", path.display());
    let bytes = std::fs::read(path).map_err(|e| ToolError::new(&ctx(), e))?;
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let mut reader = csv::Reader::from_reader(bytes);
    let headers = reader
        .headers()
        .map_err(|e| ToolError::new(&ctx(), e))?
        .clone();
    let column = |name: &str| headers.iter().position(|h| h.trim() == name);
    let file_col = column("Nazwa pliku")
        .ok_or_else(|| ToolError::new(&ctx(), "missing column `Nazwa pliku`"))?;
    let (id_col, title_col, author_col) = (column("ID"), column("Tytuł"), column("Autor"));
    let (kind_col, tags_col, text_col) = (column("Rodzaj"), column("Tagi"), column("Tekst OCR"));

    let mut rows = HashMap::new();
    for record in reader.records() {
        let record = record.map_err(|e| ToolError::new(&ctx(), e))?;
        let field = |col: Option<usize>| col.and_then(|c| record.get(c));
        let trimmed = |col| field(col).map(str::trim).filter(|v| !v.is_empty());
        let Some(file) = trimmed(Some(file_col)) else {
            continue;
        };
        let id = match trimmed(id_col) {
            Some(v) => Some(
                v.parse::<u32>()
                    .map_err(|_| ToolError::new(&ctx(), format!("bad ID `{v}` for {file}")))?,
            ),
            None => None,
        };
        let tags = field(tags_col)
            .unwrap_or("")
            .split(';')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
            .collect();
        if rows.contains_key(file) {
            return Err(ToolError::new(
                &ctx(),
                format!("duplicate `Nazwa pliku`: {file}"),
            ));
        }
        rows.insert(
            file.to_owned(),
            CsvRow {
                id,
                title: trimmed(title_col).map(str::to_owned),
                author: trimmed(author_col).map(str::to_owned),
                kind: trimmed(kind_col).map(str::to_owned),
                tags,
                text: field(text_col)
                    .filter(|t| !t.trim().is_empty())
                    .map(str::to_owned),
            },
        );
    }
    Ok(rows)
}
