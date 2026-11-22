//! Building `.notae/` for a library folder.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use notae_domain::{
    SIDECAR_DIR, SIDECAR_FILE, SIDECAR_THUMBNAILS, SIDECAR_VERSION, SidecarEntry, SidecarFile,
    parse_filename,
};
use sha2::{Digest, Sha256};

use crate::ToolError;
use crate::index_csv::{CsvRow, read_index};
use crate::thumbnails::Renderer;

const CSV_NAME: &str = "Indeks.csv";

/// What a run found. Nothing is written while `errors` is not empty.
#[derive(Debug, Default)]
pub struct Report {
    /// Problems that stop the tool from writing anything.
    pub errors: Vec<String>,
    /// Problems that do not: the sidecar is still written.
    pub warnings: Vec<String>,
}

/// Writes `.notae/library.json` and the missing thumbnails for `library`.
///
/// # Errors
/// Returns an error only for failures outside the library's content, such as
/// an unreadable folder or an unwritable `.notae/`; problems with individual
/// files are collected in the [`Report`].
pub fn run(library: &Path, renderer: &dyn Renderer) -> Result<Report, ToolError> {
    let mut report = Report::default();

    let csv_path = library.join(CSV_NAME);
    let mut csv_loaded = false;
    let mut rows = if csv_path.is_file() {
        match read_index(&csv_path) {
            Ok(rows) => {
                csv_loaded = true;
                rows
            }
            Err(e) => {
                report.errors.push(e.to_string());
                Default::default()
            }
        }
    } else {
        report
            .warnings
            .push(format!("{CSV_NAME} not found; using file names only"));
        Default::default()
    };

    let mut pdfs = Vec::new();
    collect_pdfs(library, library, &mut pdfs)?;
    pdfs.sort();

    let mut entries = Vec::new();
    for rel in &pdfs {
        let full = library.join(rel);
        let bytes = fs::read(&full).map_err(|e| ToolError::new(&format!("reading {rel}"), e))?;
        let row = rows.remove(rel.as_str());
        if row.is_none() && csv_loaded {
            report.warnings.push(format!("{rel}: no row in {CSV_NAME}"));
        }
        entries.push(entry(rel, &bytes, row.as_ref()));
    }
    let mut orphans: Vec<_> = rows.keys().cloned().collect();
    orphans.sort();
    for name in orphans {
        report
            .errors
            .push(format!("{CSV_NAME} row without a PDF: {name}"));
    }
    if !report.errors.is_empty() {
        return Ok(report);
    }

    let thumbs_dir = library.join(SIDECAR_THUMBNAILS);
    let dirs_existed = (library.join(SIDECAR_DIR).exists(), thumbs_dir.exists());
    fs::create_dir_all(&thumbs_dir).map_err(|e| ToolError::new("creating .notae", e))?;

    let wanted: BTreeSet<&str> = entries.iter().map(|e| e.sha256.as_str()).collect();
    let mut created = Vec::new();
    for entry in &entries {
        let png = thumbs_dir.join(format!("{}.png", entry.sha256));
        if png.is_file() || created.contains(&png) {
            continue;
        }
        if let Err(e) = renderer.render(&library.join(&entry.path), &png) {
            report.errors.push(format!("{}: {e}", entry.path));
            break;
        }
        created.push(png);
    }
    if !report.errors.is_empty() {
        for png in &created {
            let _ = fs::remove_file(png);
        }
        if !dirs_existed.1 {
            let _ = fs::remove_dir(&thumbs_dir);
        }
        if !dirs_existed.0 {
            let _ = fs::remove_dir(library.join(SIDECAR_DIR));
        }
        return Ok(report);
    }

    remove_stale_thumbnails(&thumbs_dir, &wanted)?;

    let file = SidecarFile {
        version: SIDECAR_VERSION,
        generator: format!("notae-sidecar {}", env!("CARGO_PKG_VERSION")),
        scores: entries,
    };
    let mut json = serde_json::to_string_pretty(&file)
        .map_err(|e| ToolError::new("serializing library.json", e))?;
    json.push('\n');
    let target = library.join(SIDECAR_FILE);
    let tmp = library.join(SIDECAR_DIR).join("library.json.tmp");
    fs::write(&tmp, json).map_err(|e| ToolError::new("writing library.json", e))?;
    fs::rename(&tmp, &target).map_err(|e| ToolError::new("replacing library.json", e))?;
    Ok(report)
}

/// Collects library-relative, `/`-separated paths of `*.pdf` files, skipping
/// `.notae/` and `sources/` at the library root.
fn collect_pdfs(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), ToolError> {
    let list =
        fs::read_dir(dir).map_err(|e| ToolError::new(&format!("listing {}", dir.display()), e))?;
    for item in list {
        let item = item.map_err(|e| ToolError::new("listing library", e))?;
        let path: PathBuf = item.path();
        let name = item.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if dir == root && (name == SIDECAR_DIR || name == "sources") {
                continue;
            }
            collect_pdfs(root, &path, out)?;
        } else if name.to_lowercase().ends_with(".pdf") {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let parts: Vec<_> = rel.iter().map(|p| p.to_string_lossy()).collect();
            out.push(parts.join("/"));
        }
    }
    Ok(())
}

fn entry(rel: &str, bytes: &[u8], row: Option<&CsvRow>) -> SidecarEntry {
    let file_name = rel.rsplit('/').next().unwrap_or(rel);
    let parsed = parse_filename(file_name);
    let row = row.cloned().unwrap_or_default();
    let mut tags: Vec<String> = Vec::new();
    for tag in row.kind.into_iter().chain(row.tags) {
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    SidecarEntry {
        path: rel.to_owned(),
        size: bytes.len() as u64,
        sha256: Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        number: row.id.or_else(|| parsed.id().map(|id| id.get())),
        title: row.title.or_else(|| Some(parsed.title().to_owned())),
        composer: row.author,
        tags,
        text: row.text,
    }
}

fn remove_stale_thumbnails(dir: &Path, wanted: &BTreeSet<&str>) -> Result<(), ToolError> {
    let list = fs::read_dir(dir).map_err(|e| ToolError::new("listing thumbnails", e))?;
    for item in list {
        let path = item
            .map_err(|e| ToolError::new("listing thumbnails", e))?
            .path();
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if path.extension().and_then(|e| e.to_str()) == Some("png") && !wanted.contains(stem) {
            fs::remove_file(&path).map_err(|e| ToolError::new("removing a stale thumbnail", e))?;
        }
    }
    Ok(())
}
