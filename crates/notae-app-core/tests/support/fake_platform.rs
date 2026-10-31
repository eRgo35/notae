//! A scripted [`PlatformLibrary`] for host tests.
//!
//! It answers every call from what the test set and logs each call, so a test can drive a
//! [`PlatformCatalog`](notae_app_core::platform::PlatformCatalog) without a platform. Include it
//! with `#[path = "support/fake_platform.rs"] mod fake_platform;`.
#![allow(dead_code)] // Each test binary uses a different part of the helper.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, MutexGuard};
use std::thread::{self, ThreadId};

use notae_app_core::platform::{DocumentContent, DocumentEntry, PlatformError, PlatformLibrary};
use notae_domain::Sha256Digest;
use sha2::{Digest, Sha256};

/// The platform's answers and the calls it received.
pub struct FakePlatform {
    root: Mutex<Result<(), PlatformError>>,
    entries: Mutex<Result<Vec<DocumentEntry>, PlatformError>>,
    text: Mutex<Result<Option<String>, PlatformError>>,
    thumbnail: Mutex<Result<bool, PlatformError>>,
    calls: Mutex<Vec<String>>,
    threads: Mutex<Vec<ThreadId>>,
    /// The next `enumerate` waits on this until the test sends or drops the sender.
    enumerate_hold: Mutex<Option<Receiver<()>>>,
    enumerate_panics: AtomicBool,
}

impl Default for FakePlatform {
    fn default() -> Self {
        Self {
            root: Mutex::new(Ok(())),
            entries: Mutex::new(Ok(Vec::new())),
            text: Mutex::new(Ok(None)),
            thumbnail: Mutex::new(Ok(false)),
            calls: Mutex::new(Vec::new()),
            threads: Mutex::new(Vec::new()),
            enumerate_hold: Mutex::new(None),
            enumerate_panics: AtomicBool::new(false),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().expect("fake platform lock is healthy")
}

impl FakePlatform {
    /// What `check_root` returns.
    pub fn set_root(&self, root: Result<(), PlatformError>) {
        *lock(&self.root) = root;
    }

    /// What the next `enumerate` returns.
    pub fn set_entries(&self, entries: Result<Vec<DocumentEntry>, PlatformError>) {
        *lock(&self.entries) = entries;
    }

    /// Makes the next `enumerate` block, as a slow Kotlin call does, until the returned sender
    /// sends or is dropped.
    pub fn hold_enumerate(&self) -> Sender<()> {
        let (release, hold) = mpsc::channel();
        *lock(&self.enumerate_hold) = Some(hold);
        release
    }

    /// Makes every later `enumerate` panic, which kills the worker thread that called it.
    pub fn panic_on_enumerate(&self) {
        self.enumerate_panics.store(true, Ordering::SeqCst);
    }

    /// What `extract_text_layer` returns.
    pub fn set_text(&self, text: Result<Option<String>, PlatformError>) {
        *lock(&self.text) = text;
    }

    /// What `render_thumbnail` returns.
    pub fn set_thumbnail(&self, rendered: Result<bool, PlatformError>) {
        *lock(&self.thumbnail) = rendered;
    }

    /// Every call so far, as `method handle [path [out]]`.
    pub fn calls(&self) -> Vec<String> {
        lock(&self.calls).clone()
    }

    /// The thread each call ran on, in call order.
    pub fn threads(&self) -> Vec<ThreadId> {
        lock(&self.threads).clone()
    }

    fn record(&self, call: String) {
        lock(&self.calls).push(call);
        lock(&self.threads).push(thread::current().id());
    }
}

impl PlatformLibrary for FakePlatform {
    fn check_root(&self, handle: String) -> Result<(), PlatformError> {
        self.record(format!("check_root {handle}"));
        lock(&self.root).clone()
    }

    fn enumerate(&self, handle: String) -> Result<Vec<DocumentEntry>, PlatformError> {
        self.record(format!("enumerate {handle}"));
        let hold = lock(&self.enumerate_hold).take();
        if let Some(hold) = hold {
            let _ = hold.recv();
        }
        assert!(
            !self.enumerate_panics.load(Ordering::SeqCst),
            "the fake platform was told to panic in enumerate"
        );
        lock(&self.entries).clone()
    }

    fn extract_text_layer(
        &self,
        handle: String,
        path: String,
    ) -> Result<Option<String>, PlatformError> {
        self.record(format!("text {handle} {path}"));
        lock(&self.text).clone()
    }

    fn render_thumbnail(
        &self,
        handle: String,
        path: String,
        out_png: String,
    ) -> Result<bool, PlatformError> {
        self.record(format!("thumbnail {handle} {path} {out_png}"));
        lock(&self.thumbnail).clone()
    }
}

/// The hex SHA-256 a platform reports for `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256Digest::from_bytes(Sha256::digest(bytes).into()).to_string()
}

/// A readable entry whose display name is the path's last segment.
pub fn readable(path: &str, bytes: &[u8]) -> DocumentEntry {
    entry(
        path,
        DocumentContent::Sha256 {
            hex: sha256_hex(bytes),
        },
        bytes.len(),
    )
}

/// An entry with `content` and `size` bytes.
pub fn entry(path: &str, content: DocumentContent, size: usize) -> DocumentEntry {
    DocumentEntry {
        path: path.to_owned(),
        display_name: path.rsplit('/').next().unwrap_or(path).to_owned(),
        size: u64::try_from(size).expect("fixture size fits in u64"),
        content,
    }
}
