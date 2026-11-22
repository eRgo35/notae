//! Rendering the first page of a PDF to a PNG.

use std::path::Path;
use std::process::Command;

/// Renders the first page of a PDF to a PNG file.
pub trait Renderer {
    /// Writes the first page of `pdf` as a PNG to exactly `out_png`.
    ///
    /// # Errors
    /// Returns a message describing why rendering failed.
    fn render(&self, pdf: &Path, out_png: &Path) -> Result<(), String>;
}

/// The real renderer: the `pdftoppm` command line tool.
pub struct Pdftoppm;

impl Renderer for Pdftoppm {
    fn render(&self, pdf: &Path, out_png: &Path) -> Result<(), String> {
        let output = Command::new("pdftoppm")
            .args([
                "-png",
                "-f",
                "1",
                "-l",
                "1",
                "-scale-to-x",
                "400",
                "-scale-to-y",
                "-1",
            ])
            .arg("-singlefile")
            .arg(pdf)
            .arg(out_png.with_extension(""))
            .output()
            .map_err(|e| format!("cannot run pdftoppm: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "pdftoppm failed ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }
}
