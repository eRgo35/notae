//! A [`PdfCatalog`] over an ordinary Linux directory tree.
//!
//! This is the only place in the CLI that knows about `std::fs`. Everything
//! above it consumes [`PdfFile`] values, so the Android Storage Access
//! Framework adapter and this desktop walk are interchangeable.
//!
//! # The library root boundary
//!
//! Three rules keep a run inside the root it was given.
//!
//! * **Symbolic links are never followed, including at the root.** A symlinked
//!   file, including one that resolves to a target still inside the root, is
//!   skipped and reported as a [`CatalogFailure`]; a symlinked directory is not
//!   walked. Skipping unconditionally — rather than canonicalising the target
//!   and comparing it with the canonical root — is what makes the guarantee
//!   hold: the link is never opened, so no bytes are read through it, and a
//!   symlink loop terminates because the walk never follows a link in the first
//!   place. An entry that *becomes* a link after the listing recorded it is
//!   caught by the identity check described below rather than by this rule.
//! * **A symbolic link *as* the root is refused, not followed.** The rule above
//!   has to reach the root itself or it means nothing: the root is the one name
//!   a user hands the tool, so following a link there would let the run be
//!   silently retargeted at a tree the user never selected, and would defeat
//!   the guarantee for every entry below it. [`ValidatedRoot::open`]
//!   therefore rejects a symlinked root by name, with a message that says so
//!   and tells the user to pass the directory it points at. The root is
//!   canonicalised exactly zero times.
//!
//!   What counts as "the name" is fixed by [`named_root`], because a name can
//!   name a link while spelling something else: Linux resolves a trailing
//!   separator — and a trailing `.`, and a doubled separator — before it
//!   stats anything, so a check on `link/` describes the link's *target* and
//!   the link itself goes unseen. [`named_root`] rebuilds the name from its
//!   [`Path::components`], which drops exactly those spellings and nothing
//!   else. The rebuild is lexical: no component is resolved and nothing is
//!   canonicalised, so the name the user typed is still the name inspected,
//!   only its final component is inspected *as itself* rather than as whatever
//!   a trailing separator points at. `link`, `link/`, `link/.`, `link//` and
//!   `link/./` are one root and one check; a real directory spelled any of those
//!   ways is accepted unchanged.
//! * **The validated root is held, not just remembered.** Refusing a link at
//!   the root only settles the instant of validation; between that check and
//!   the walk the name could still be retargeted. The check therefore does not
//!   merely *judge* the name and hand it back — it *is* the object the run
//!   uses. [`ValidatedRoot::open`] is called once per run and returns the
//!   directory it opened: the name, an open handle, and the device and inode
//!   that handle has. [`FilesystemCatalog::new`] takes that object rather than
//!   a path, so there is no second lookup that could accept a different
//!   directory, and [`FilesystemCatalog::enumerate`] will only use the name
//!   while it still resolves to that same device and inode — so a root
//!   replaced by a link, or by a different directory, ends the run instead of
//!   quietly indexing something else. Holding the handle also keeps the inode
//!   alive, so its identity cannot be handed to another directory while the
//!   run lasts. The same identity check runs on each subdirectory before the
//!   walk descends and on each file before its bytes are read, so the guarantee
//!   covers the whole traversal rather than only its first step.
//!
//!   What that does *not* claim: the checks are `stat` calls and the operations
//!   they guard are separate path resolutions, so a run is not atomic against a
//!   process that substitutes an entry in the microseconds between the check
//!   and the use. Closing that last window needs `openat` with `O_NOFOLLOW`
//!   and `fdopendir`, which would mean `unsafe` or a syscall-level dependency
//!   in a CLI that has neither; against a user indexing their own library on
//!   their own machine, the substituted object is one they could equally have
//!   selected. The check is a defence against silent redirection, not a
//!   privilege boundary, and is documented as such rather than overstated.
//! * **A name the domain type cannot represent is a reported skip.** POSIX
//!   permits filenames the portable [`RelativePdfPath`] rejects, such as
//!   `C:.pdf`, whose leading `X:` would be a Windows drive prefix in a
//!   sidecar. Those files are not indexed, because indexing them would put a
//!   non-portable path in the sidecar; they are recorded as
//!   [`CatalogFailure`]s and the rest of the batch continues. The domain
//!   invariant stays strict and the catalog degrades, so one unusual Linux
//!   filename costs one file rather than the whole run.
//!
//! A failure to read the root is not a [`CatalogFailure`] at all. The root is
//! the boundary of the run: if it cannot be opened, nothing below it was ever
//! seen, so there is no report to degrade and the run fails instead. The same
//! is true of an unreadable *subdirectory*, but only for that directory — the
//! rest of the tree is still enumerated and still reported.

use std::cell::RefCell;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use notae_domain::{PdfFingerprint, RelativePdfPath, Sha256Digest};
use notae_index::{CatalogError, FileCondition, PdfCatalog, PdfFile};
use sha2::{Digest, Sha256};

/// Read buffer for hashing; large enough that hashing is not syscall bound.
const READ_BUFFER: usize = 64 * 1024;

/// One traversal problem that did not stop enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogFailure {
    /// The path that could not be visited or read, relative to the root when
    /// it is inside it.
    path: String,
    /// The operating system's description of the problem.
    error: String,
}

impl CatalogFailure {
    /// The path the failure concerns.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The underlying error message.
    #[must_use]
    pub fn error(&self) -> &str {
        &self.error
    }
}

/// Records one traversal problem.
///
/// The path is shown relative to `root` when it lies inside it, so reports read
/// the same way as the paths that succeeded. The root itself has no relative
/// form — [`Path::strip_prefix`] yields an empty path for it — so an empty
/// result falls back to the root as given, which is what identifies the thing
/// that failed.
fn push_failure(
    failures: &RefCell<Vec<CatalogFailure>>,
    root: &Path,
    path: &Path,
    error: impl Into<String>,
) {
    let relative = path.strip_prefix(root).map_or_else(
        |_| path.display().to_string(),
        |inside| {
            let inside = inside.display().to_string();
            if inside.is_empty() {
                root.display().to_string()
            } else {
                inside
            }
        },
    );
    failures.borrow_mut().push(CatalogFailure {
        path: relative,
        error: error.into(),
    });
}

/// The object a path resolved to when it was vetted.
///
/// Unix names an object by its device and inode, and those two numbers are what
/// prove that a name still resolves to the same thing later. On platforms that
/// do not expose them the identity is empty and every comparison succeeds,
/// which is exactly as much as such a platform can promise.
#[cfg(unix)]
type Identity = (u64, u64);
#[cfg(not(unix))]
type Identity = ();

/// The identity of the object `metadata` describes.
#[cfg(unix)]
fn identity_of(metadata: &std::fs::Metadata) -> Identity {
    use std::os::unix::fs::MetadataExt;
    (metadata.dev(), metadata.ino())
}

#[cfg(not(unix))]
fn identity_of(metadata: &std::fs::Metadata) -> Identity {
    let _ = metadata;
}

/// Why a library root name was refused before a run started.
///
/// The distinction is the one a user can act on: a name that *is* a link is
/// refused as a link, with the directory it points at named as the thing to
/// pass instead, while a name that is simply absent, is not a directory, or
/// cannot be read is refused as unusable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootRejection {
    /// The final component of the name is a symbolic link, which is never
    /// followed.
    Symlink(PathBuf),
    /// The name is absent, is not a directory, or cannot be listed or opened.
    Unusable {
        /// The name that was refused, as it will be spelled in the message.
        path: PathBuf,
        /// What the operating system said.
        reason: String,
    },
}

impl fmt::Display for RootRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Symlink(path) => write!(
                formatter,
                "{} is a symbolic link, which is never followed; pass the directory it points at",
                path.display()
            ),
            Self::Unusable { path, reason } => {
                write!(formatter, "{}: {reason}", path.display())
            }
        }
    }
}

/// The library root of a run: the name, the directory it was opened as, and
/// that directory's identity.
///
/// This is the single object that answers "which tree is this run about". It
/// exists so that the answer cannot drift: the run checks the name once, here,
/// and everything downstream walks the directory this object holds rather than
/// resolving a path again. A caller that wants to know whether a root is usable
/// therefore does not get a verdict and a string — it gets the opened
/// directory, which is the only thing worth walking.
#[derive(Debug)]
pub struct ValidatedRoot {
    /// The name as [`named_root`] spells it, which is the spelling every later
    /// use of the root goes through.
    name: PathBuf,
    /// The opened directory, kept alive so its identity cannot be reused by
    /// another directory while the run lasts. Nothing reads from it — that is
    /// the whole of what it is for.
    _handle: File,
    /// The device and inode the open handle refers to; see [`verify_root`].
    identity: Identity,
}

impl ValidatedRoot {
    /// Checks a library root name and opens the directory it names.
    ///
    /// This is the run's one and only root check, and it happens here, on the
    /// object the run then uses. Three things are settled, in the order a user
    /// would fix them:
    ///
    /// * [`named_root`] first strips the spellings Linux resolves away before
    ///   it stats anything — a trailing separator, a trailing `.`, a doubled
    ///   separator — so the name inspected below is the name the user typed
    ///   with only those removed.
    /// * [`std::fs::symlink_metadata`] inspects that name itself, so a root
    ///   that is a symbolic link is refused by name rather than followed.
    /// * [`std::fs::read_dir`] proves the directory's entries can be listed at
    ///   all, which is what a run actually needs from its root and what
    ///   `metadata` cannot tell us: a directory with no read permission still
    ///   answers `stat`. [`File::open`] then keeps the directory itself open
    ///   for the rest of the run, and its identity — taken from the handle,
    ///   not from the name — is the authority [`FilesystemCatalog::enumerate`]
    ///   checks against.
    ///
    /// # Errors
    /// Returns [`RootRejection::Symlink`] when the name is a symbolic link and
    /// [`RootRejection::Unusable`] when it is absent, is not a directory, or
    /// cannot be listed or opened; both name the root, so the message
    /// identifies the thing the user has to fix.
    pub fn open(root: &Path) -> Result<Self, RootRejection> {
        let name = named_root(root);
        let unusable = |reason: String| RootRejection::Unusable {
            path: name.clone(),
            reason,
        };
        let metadata =
            std::fs::symlink_metadata(&name).map_err(|error| unusable(error.to_string()))?;
        if metadata.file_type().is_symlink() {
            return Err(RootRejection::Symlink(name));
        }
        if !metadata.is_dir() {
            return Err(unusable("not a directory".to_owned()));
        }
        // Listing the root is the point of the check, not a side effect: an
        // unreadable root means nothing below it was ever seen, so there is no
        // report to degrade into per-file skips.
        std::fs::read_dir(&name).map_err(|error| unusable(error.to_string()))?;
        let handle = File::open(&name).map_err(|error| unusable(error.to_string()))?;
        let opened = handle
            .metadata()
            .map_err(|error| unusable(error.to_string()))?;
        Ok(Self {
            name,
            _handle: handle,
            identity: identity_of(&opened),
        })
    }

    /// The root name, spelled the way every later use of it will be.
    #[must_use]
    pub fn name(&self) -> &Path {
        &self.name
    }

    /// The identity of the directory this root was opened as.
    ///
    /// Taken from the open handle rather than from the name, so it describes
    /// the directory the run holds even after the name has been moved aside.
    fn identity(&self) -> Identity {
        self.identity
    }
}

/// The library root with the spellings Linux resolves away removed.
///
/// A trailing separator, a trailing `.`, and a doubled separator are not
/// different directories: the kernel resolves all three before it stats
/// anything, so [`std::fs::symlink_metadata`] on `link/` describes the target
/// of the link and the link itself is never inspected — which is exactly the
/// hole the root boundary cannot have. Rebuilding the name from its
/// [`Path::components`] drops those and nothing else.
///
/// The rebuild is purely lexical: no component is resolved, no path is
/// canonicalised, and the filesystem is not consulted, so the spelling the user
/// typed is still the spelling that gets inspected — with the final component
/// inspected as itself rather than as whatever a trailing separator pointed at.
/// Only the final component is inspected by name anywhere; intermediate
/// components are resolved by the kernel exactly as they always were.
fn named_root(root: &Path) -> PathBuf {
    root.components().collect()
}

/// Confirms the root name still resolves to the directory the run opened.
///
/// This is the check that spans the gap between validation and enumeration:
/// the handle was opened on the validated directory, so its identity is the
/// answer to "which directory is this run about", and the name is only
/// accepted while it still denotes that same object. A root that has been
/// replaced, renamed away, or turned into a symbolic link fails the run rather
/// than redirecting it at a tree the user never selected. `root` is expected
/// to have been through [`named_root`], so this cannot be fooled by a trailing
/// separator either.
///
/// # Errors
/// Returns [`CatalogError::Unreadable`] naming `root` when the name is gone,
/// unreadable, a symbolic link, or no longer the directory that was opened.
fn verify_root(root: &Path, identity: Identity) -> Result<(), CatalogError> {
    let metadata = std::fs::symlink_metadata(root)
        .map_err(|error| CatalogError::Unreadable(format!("{}: {error}", root.display())))?;
    if metadata.file_type().is_symlink() {
        return Err(CatalogError::Unreadable(format!(
            "{} is now a symbolic link, which is never followed; \
             pass the directory it points at",
            root.display()
        )));
    }
    if identity_of(&metadata) != identity {
        return Err(CatalogError::Unreadable(format!(
            "{} is no longer the directory this run opened; it was replaced \
             while the run was in progress, so the tree cannot be trusted — \
             re-run against the library you meant to index",
            root.display()
        )));
    }
    Ok(())
}

/// One path the walk collected, and the object that path named when it did.
///
/// Carrying the identity alongside the path is what lets the caller prove, at
/// the moment it acts on the name, that it is still the entry that was vetted.
type Collected = (String, Identity);

/// Enumerates the PDFs under one library root.
///
/// Per-file problems — a file that cannot be opened, a *subdirectory* that
/// cannot be read — are recorded in [`failures`](Self::failures) and the
/// corresponding entry is still enumerated as [`FileCondition::Unreadable`]
/// where the path is known, so one bad PDF cannot abort a batch. Entries that
/// are skipped outright — a symlink, an entry replaced under the name the walk
/// recorded, or a name the portable path type rejects — appear in
/// [`failures`](Self::failures) only; see the module documentation for the
/// root-boundary rules that decide this. A root that no longer names the
/// directory this catalog was handed is neither: it is an error, because no
/// part of the tree was seen.
#[derive(Debug)]
pub struct FilesystemCatalog {
    /// The validated root: its name, and the directory that name was opened
    /// as. Holding both is what lets the walk use the name without trusting it.
    root: ValidatedRoot,
    failures: RefCell<Vec<CatalogFailure>>,
}

impl FilesystemCatalog {
    /// Builds a catalog over an already-opened library root.
    ///
    /// This cannot fail, and that is the point: the root was checked once,
    /// when [`ValidatedRoot::open`] opened it, and there is no second lookup
    /// here that could accept a different directory. What the root can still
    /// do is change underneath the run, which
    /// [`enumerate`](PdfCatalog::enumerate) catches before it uses the name.
    #[must_use]
    pub fn new(root: ValidatedRoot) -> Self {
        Self {
            root,
            failures: RefCell::new(Vec::new()),
        }
    }

    /// The library root this catalog walks.
    #[must_use]
    pub fn root(&self) -> &Path {
        self.root.name()
    }

    /// Every traversal problem recorded since the catalog was opened.
    #[must_use]
    pub fn failures(&self) -> Vec<CatalogFailure> {
        self.failures.borrow().clone()
    }

    fn record(&self, path: &Path, error: &io::Error) {
        push_failure(&self.failures, self.root(), path, error.to_string());
    }

    fn record_message(&self, path: &Path, error: impl Into<String>) {
        push_failure(&self.failures, self.root(), path, error);
    }
}

impl PdfCatalog for FilesystemCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        // The root name is checked against the directory this catalog was
        // opened on before it is used for anything. Without this the walk
        // would resolve the name again and follow whatever it points at now.
        let root = self.root.name();
        verify_root(root, self.root.identity())?;

        // The root is opened here rather than inside the walk so that failing
        // to list it ends the run. A failure deeper in the tree is recorded
        // and the walk continues; a failure at the root means the tree the
        // caller asked for was never seen, so there is no report to trust.
        let entries = std::fs::read_dir(root)
            .map_err(|error| CatalogError::Unreadable(format!("{}: {error}", root.display())))?;

        let mut collected = Vec::new();
        collect_pdf_paths(root, root, entries, "", &mut collected, &self.failures);
        collected.sort();

        let files = collected
            .into_iter()
            .filter_map(|(relative, identity)| {
                let filename = relative
                    .rsplit('/')
                    .next()
                    .unwrap_or(relative.as_str())
                    .to_owned();
                // A name POSIX allows but the portable path type refuses is a
                // reported skip, not a panic: one file is lost, not the batch.
                let path = match RelativePdfPath::new(&relative) {
                    Ok(path) => path,
                    Err(error) => {
                        self.record_message(Path::new(&relative), error.to_string());
                        return None;
                    }
                };
                let absolute = root.join(path.as_str());
                Some(match fingerprint(&absolute, identity) {
                    Ok(fingerprint) => PdfFile::with_condition(
                        path,
                        filename,
                        FileCondition::Readable(fingerprint),
                    ),
                    Err(error) => {
                        self.record(&absolute, &error);
                        PdfFile::with_condition(path, filename, FileCondition::Unreadable)
                    }
                })
            })
            .collect();
        Ok(files)
    }
}

/// Collects every `.pdf`/`.PDF` path below the already-opened `entries`,
/// as `prefix`-joined forward-slash strings relative to the library root, each
/// paired with the identity of the entry it names.
///
/// Symbolic links are skipped without being followed; see the module
/// documentation for why. `DirEntry::file_type` does not traverse a link, so
/// this holds even for a loop or a link that leaves the root.
///
/// A directory's recorded identity is checked against the name immediately
/// before the walk descends, because the entry type comes from the directory
/// listing and the descent resolves the name afresh: without the check, a
/// directory replaced by a link in between would be walked through. A
/// subdirectory that cannot be opened, or that has changed identity, is
/// recorded and skipped: the walk continues, because one inaccessible or
/// replaced directory does not invalidate what the rest of the tree showed.
/// The root is opened by the caller for the opposite reason — see
/// [`FilesystemCatalog::enumerate`].
fn collect_pdf_paths(
    library_root: &Path,
    dir: &Path,
    entries: std::fs::ReadDir,
    prefix: &str,
    out: &mut Vec<Collected>,
    failures: &RefCell<Vec<CatalogFailure>>,
) {
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                push_failure(failures, library_root, dir, error.to_string());
                continue;
            }
        };
        let path = entry.path();
        let name = entry.file_name();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                push_failure(failures, library_root, &path, error.to_string());
                continue;
            }
        };
        if file_type.is_symlink() {
            push_failure(
                failures,
                library_root,
                &path,
                format!("{}: symbolic link skipped", name.to_string_lossy()),
            );
            continue;
        }
        let joined = if prefix.is_empty() {
            name.to_string_lossy().into_owned()
        } else {
            format!("{prefix}/{}", name.to_string_lossy())
        };
        // Only an entry the walk will act on needs the extra `stat`: `file_type`
        // came from the listing, and only the identity says whether the name
        // still resolves to that same object.
        let is_pdf = is_pdf(&name);
        if !file_type.is_dir() && !is_pdf {
            continue;
        }
        // The listing described the entry as it was a moment ago; the identity
        // it recorded is what the walk acts on, not the name.
        let identity = match entry.metadata() {
            Ok(metadata) => identity_of(&metadata),
            Err(error) => {
                push_failure(failures, library_root, &path, error.to_string());
                continue;
            }
        };
        if file_type.is_dir() {
            // A subdirectory that cannot be opened, or that has changed
            // identity, is recorded and left behind: the rest of the tree still
            // describes a real library.
            match open_subdirectory(&path, identity) {
                Ok(nested_entries) => {
                    collect_pdf_paths(library_root, &path, nested_entries, &joined, out, failures);
                }
                Err(error) => push_failure(failures, library_root, &path, error.to_string()),
            }
            continue;
        }
        out.push((joined, identity));
    }
}

/// Opens a subdirectory for the walk to descend into.
///
/// Descending resolves the name again, so the directory the listing described
/// is confirmed to still be behind it first: a name that has become a symbolic
/// link, or that now names a different object, is refused rather than walked.
/// [`std::fs::symlink_metadata`] inspects the name itself, so a link is refused
/// on its own identity rather than followed to its target.
fn open_subdirectory(path: &Path, identity: Identity) -> io::Result<std::fs::ReadDir> {
    let same = std::fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink() && identity_of(&metadata) == identity
    });
    if !same {
        return Err(io::Error::other(
            "the directory changed identity while the walk was running",
        ));
    }
    std::fs::read_dir(path)
}

/// Whether a directory entry name names a PDF, case-insensitively.
fn is_pdf(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| name.len() >= 4 && name[name.len() - 4..].eq_ignore_ascii_case(".pdf"))
}

/// Hashes one file end to end and collects its platform metadata.
///
/// `identity` is what the walk recorded for this name when it collected it, and
/// this is the last line of defence for the root boundary. The walk already
/// refuses to collect a symbolic link, but a regular file can be replaced
/// between collection and this call — by a link, or by another file under the
/// same name. [`std::fs::symlink_metadata`] inspects the name itself, so a link
/// is refused on its own identity rather than followed, and comparing the
/// identity of the handle that was actually opened against `identity` proves
/// the bytes came from the entry that was vetted. The check costs two `stat`
/// calls and no syscall beyond them.
///
/// # Errors
/// Returns the I/O error when the file is a symbolic link, is no longer the
/// entry that was collected, or cannot be opened or read, so the caller can
/// record it and enumerate the file as unreadable.
fn fingerprint(path: &Path, identity: Identity) -> Result<PdfFingerprint, io::Error> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other("symbolic link skipped"));
    }
    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    if identity_of(&opened) != identity {
        return Err(io::Error::other(
            "the entry changed identity while it was being read",
        ));
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; READ_BUFFER];
    let mut byte_len = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        byte_len += u64::try_from(read).expect("a chunk length fits in u64");
    }
    let digest = hasher.finalize();
    let sha256 = Sha256Digest::from_bytes(digest.into());
    PdfFingerprint::from_system_time(sha256, byte_len, metadata.modified().ok())
        .ok_or_else(|| io::Error::other("the file's modification time is out of range"))
}

#[cfg(test)]
mod tests {
    use super::{fingerprint, identity_of, named_root, open_subdirectory, verify_root};
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn identity(path: &std::path::Path) -> super::Identity {
        identity_of(&fs::symlink_metadata(path).expect("stat the entry"))
    }

    /// The spellings Linux resolves away before it stats anything.
    ///
    /// These are the spellings that make a link invisible to a check: `link/`
    /// is not "the link's target" as a rule of this tool, it is the link, and
    /// [`named_root`] is what says so. The rebuild is lexical, so what it must
    /// not do is alter any other part of the name.
    #[test]
    fn named_root_drops_only_the_spellings_the_kernel_resolves_away() {
        for (given, expected) in [
            ("link", "link"),
            ("link/", "link"),
            ("link/.", "link"),
            ("link//", "link"),
            ("link/./", "link"),
            ("/a//b/", "/a/b"),
            ("/a/b/./", "/a/b"),
            ("/a/b/c", "/a/b/c"),
            ("relative/link/", "relative/link"),
        ] {
            assert_eq!(
                named_root(Path::new(given)),
                PathBuf::from(expected),
                "`{given}` names `{expected}`"
            );
        }
    }

    #[test]
    fn named_root_leaves_names_it_has_nothing_to_say_about() {
        for given in ["/", ".", "..", "/a/../b"] {
            assert_eq!(
                named_root(Path::new(given)),
                PathBuf::from(given),
                "`{given}` is passed through untouched"
            );
        }
    }

    /// The checks that run *between* collecting an entry and acting on it.
    ///
    /// Winning the timing race that motivates them from an integration test is
    /// not something a test can rely on, so these drive the mismatch directly:
    /// an identity recorded before a substitution is presented to the check that
    /// has to notice it. Both are the same code path the walk takes, so a
    /// regression in either shows up here.
    #[test]
    fn fingerprint_refuses_an_entry_replaced_under_the_vetted_identity() {
        let dir = TempDir::new().expect("temporary directory");
        let path = dir.path().join("book.pdf");
        fs::write(&path, "original").expect("write the original");
        let vetted = identity(&path);

        let replacement = dir.path().join("replacement.pdf");
        fs::write(&replacement, "substituted").expect("write the substitute");
        fs::rename(&replacement, &path).expect("swap the file under the same name");

        let error = fingerprint(&path, vetted).expect_err("the substitute is not the vetted file");
        assert!(
            error.to_string().contains("changed identity"),
            "the refusal says the entry was replaced: {error}"
        );
    }

    #[test]
    fn fingerprint_still_reads_the_entry_its_own_identity_names() {
        let dir = TempDir::new().expect("temporary directory");
        let path = dir.path().join("book.pdf");
        fs::write(&path, "content").expect("write the file");

        let fingerprint = fingerprint(&path, identity(&path)).expect("the vetted file is read");
        assert_eq!(fingerprint.byte_len(), 7);
    }

    #[test]
    fn a_subdirectory_is_not_opened_once_it_is_replaced() {
        let dir = TempDir::new().expect("temporary directory");
        let nested = dir.path().join("nested");
        fs::create_dir(&nested).expect("create the directory");
        let vetted = identity(&nested);

        assert!(
            open_subdirectory(&nested, vetted).is_ok(),
            "an untouched directory is still the one that was listed"
        );
        fs::remove_dir(&nested).expect("remove it");
        fs::create_dir(&nested).expect("create another directory at the same name");

        let Err(error) = open_subdirectory(&nested, vetted) else {
            panic!("a different directory behind the same name must be refused");
        };
        assert!(
            error.to_string().contains("changed identity"),
            "the refusal says the directory was replaced: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_subdirectory_is_not_opened_once_it_becomes_a_link() {
        let dir = TempDir::new().expect("temporary directory");
        let nested = dir.path().join("nested");
        fs::create_dir(&nested).expect("create the directory");
        let vetted = identity(&nested);

        let target = dir.path().join("target");
        fs::create_dir(&target).expect("create the link target");
        fs::write(target.join("secret.pdf"), "outside").expect("write behind the link");
        fs::remove_dir(&nested).expect("remove the directory");
        std::os::unix::fs::symlink(&target, &nested).expect("replace it with a link");

        let Err(error) = open_subdirectory(&nested, vetted) else {
            panic!("a link is refused on its own identity rather than followed");
        };
        assert!(
            error.to_string().contains("changed identity"),
            "the refusal says the entry was replaced: {error}"
        );
    }

    #[test]
    fn verify_root_refuses_a_name_that_no_longer_denotes_the_opened_directory() {
        let dir = TempDir::new().expect("temporary directory");
        let root = dir.path().join("library");
        fs::create_dir(&root).expect("create the root");
        let vetted = identity(&root);

        assert!(
            verify_root(&root, vetted).is_ok(),
            "the root as opened still verifies"
        );

        let moved = dir.path().join("moved");
        fs::rename(&root, &moved).expect("move the root aside");
        fs::create_dir(&root).expect("create another directory at the root name");

        let error = verify_root(&root, vetted).expect_err("the substituted root is refused");
        let message = error.to_string();
        assert!(
            message.contains(&root.display().to_string()),
            "the refusal names the root as given: {message}"
        );
        assert!(
            message.contains("no longer the directory this run opened"),
            "the refusal says the root changed under the run: {message}"
        );
    }
}
