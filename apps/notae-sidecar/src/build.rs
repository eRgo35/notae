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
/// Scratch directory for new thumbnails, removed at the end of every run.
const STAGING_DIR: &str = ".notae-staging";

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
    let mut rows = match read_index(&csv_path) {
        Ok(rows) => rows,
        Err(e) => {
            report.errors.push(e.to_string());
            return Ok(report);
        }
    };

    let mut pdfs = Vec::new();
    collect_pdfs(library, library, &mut pdfs)?;
    pdfs.sort();

    let mut entries = Vec::new();
    for rel in &pdfs {
        let full = library.join(rel);
        let bytes = fs::read(&full).map_err(|e| ToolError::new(&format!("reading {rel}"), e))?;
        let row = rows.remove(rel.as_str());
        if row.is_none() {
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

    // Render what is missing into a staging directory; .notae/ is not touched
    // until every render has succeeded.
    let thumbs_dir = library.join(SIDECAR_THUMBNAILS);
    let staging = library.join(STAGING_DIR);
    let _ = fs::remove_dir_all(&staging);
    let outcome = write_sidecar(
        library,
        &thumbs_dir,
        &staging,
        entries,
        renderer,
        &mut report,
    );
    let _ = fs::remove_dir_all(&staging);
    outcome?;
    Ok(report)
}

fn write_sidecar(
    library: &Path,
    thumbs_dir: &Path,
    staging: &Path,
    entries: Vec<SidecarEntry>,
    renderer: &dyn Renderer,
    report: &mut Report,
) -> Result<(), ToolError> {
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
    for entry in &entries {
        let target = thumbs_dir.join(format!("{}.png", entry.sha256));
        let temp = staging.join(format!("{}.png", entry.sha256));
        if target.is_file() || staged.iter().any(|(t, _)| *t == temp) {
            continue;
        }
        if staged.is_empty() && !staging.exists() {
            fs::create_dir(staging).map_err(|e| ToolError::new("creating staging directory", e))?;
        }
        let rendered = renderer
            .render(&library.join(&entry.path), &temp)
            .and_then(|()| match fs::metadata(&temp) {
                Ok(m) if m.is_file() && m.len() > 0 => Ok(()),
                _ => Err("renderer produced no output file".to_owned()),
            });
        match rendered {
            Ok(()) => staged.push((temp, target)),
            Err(e) => report.errors.push(format!("{}: {e}", entry.path)),
        }
    }
    if !report.errors.is_empty() {
        return Ok(());
    }

    fs::create_dir_all(thumbs_dir).map_err(|e| ToolError::new("creating .notae", e))?;
    for (temp, target) in &staged {
        fs::rename(temp, target).map_err(|e| ToolError::new("moving a thumbnail", e))?;
    }
    let wanted: BTreeSet<&str> = entries.iter().map(|e| e.sha256.as_str()).collect();
    remove_stale_thumbnails(thumbs_dir, &wanted)?;

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
    fs::rename(&tmp, &target).map_err(|e| ToolError::new("replacing library.json", e))
}

/// Folders at the library root that hold no library scores: transcription
/// sources, original scans and other material.
const SKIPPED_ROOT_DIRS: [&str; 3] = ["sources", "skany", "inne"];

/// Collects library-relative, `/`-separated paths of `*.pdf` files, skipping
/// `.notae/` and [`SKIPPED_ROOT_DIRS`] at the library root.
fn collect_pdfs(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), ToolError> {
    let list =
        fs::read_dir(dir).map_err(|e| ToolError::new(&format!("listing {}", dir.display()), e))?;
    for item in list {
        let item = item.map_err(|e| ToolError::new("listing library", e))?;
        let path: PathBuf = item.path();
        let name = item.file_name().to_string_lossy().into_owned();
        let Ok(kind) = item.file_type() else { continue };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if dir == root && (name == SIDECAR_DIR || SKIPPED_ROOT_DIRS.contains(&name.as_str())) {
                continue;
            }
            collect_pdfs(root, &path, out)?;
        } else if kind.is_file() && name.to_lowercase().ends_with(".pdf") {
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
    // A row's empty `ID` stays null; only a PDF with no row takes its file name's number.
    let number = match row {
        Some(row) => row.id,
        None => parsed.id().map(|id| id.get()),
    };
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
        number,
        title: row.title.or_else(|| Some(parsed.title().to_owned())),
        composer: row.author,
        tags,
        text: row.text,
    }
}

/// Removes regular files named exactly `<64 lowercase hex>.png` whose hash is
/// no longer wanted; anything else in the directory is not ours.
fn remove_stale_thumbnails(dir: &Path, wanted: &BTreeSet<&str>) -> Result<(), ToolError> {
    let list = fs::read_dir(dir).map_err(|e| ToolError::new("listing thumbnails", e))?;
    for item in list {
        let item = item.map_err(|e| ToolError::new("listing thumbnails", e))?;
        let name = item.file_name().to_string_lossy().into_owned();
        let Some(hash) = name.strip_suffix(".png") else {
            continue;
        };
        let is_hash =
            hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        let is_file = item.file_type().map(|t| t.is_file()).unwrap_or(false);
        if is_hash && is_file && !wanted.contains(hash) {
            fs::remove_file(item.path())
                .map_err(|e| ToolError::new("removing a stale thumbnail", e))?;
        }
    }
    Ok(())
}
