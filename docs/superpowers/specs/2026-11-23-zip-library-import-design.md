# Score library from a ZIP file

- **Date:** 2026-11-23
- **Release:** v0.9.0, with the viewer layout and zoom work.

## Intent

A choir gets its scores as one ZIP, usually from a link the conductor sends (a web server or a cloud drive).
Picking a folder through Android's folder picker is the hard part for these users: they have to find the
folder, the picker refuses roots such as Download, and they are asked about access to the SD card. With a ZIP,
Notae keeps the scores in its own storage and asks nothing. Notae itself stays offline: it never gets the
`INTERNET` permission (ADR 0001–0004, `PRIVACY.md`). The browser downloads and Notae imports the downloaded file.

## Decisions (maintainer)

- **The ZIP is the library.** Importing a ZIP is an alternative to "Choose score folder". The scores land in
  Notae's private storage and become the one library. Choosing a folder later switches back, exactly as changing
  folders does today.
- **A new ZIP replaces the files and keeps the rest.** Importing again replaces the stored PDFs with the new ZIP's
  and rescans. Notes, favorites, sets and drawings stay with the scores that still exist, as when PDFs change in a
  folder today.
- **"From a link" opens the browser** with an address typed or pasted in Notae, with steps saying what to tap.
  Then the user picks the downloaded file, or shares it to Notae.

## Goals

1. **Import scores from a ZIP** on the welcome screen and next to Change in Settings → Library folder. It opens a
   sheet with:
   - **Choose ZIP file**: the system file picker on ZIP files.
   - **From a link**: an address field and **Open in browser**, with two steps: "1. Your browser downloads
     the ZIP (in Google Drive: ⋮ → Download). 2. Come back and tap Choose ZIP file, or share the file to Notae."
2. **Share → Notae** and **Open with → Notae** on a ZIP import it. When a library is already open, Notae first
   asks "Replace your library with the scores in “X.zip”?". On the welcome screen it imports at once.
3. While it unpacks, a modal "Unpacking “X.zip”…" shows. Then the normal scan banner takes over.
4. Errors leave the current library untouched and show what went wrong: not a ZIP, no PDFs in it, not enough
   space, or unreadable.

## Non-goals

- Downloading inside Notae (no `INTERNET`).
- Two libraries at once, or merging a ZIP into a folder library.
- Passwords, RAR or 7z.

## Approaches

1. **Unpack into private storage behind a `file:` handle (picked).** Kotlin unpacks into
   `filesDir/zip-library/` and reports it to the core as a picked library root: name = ZIP name, handle =
   that directory's `file:` URI. The platform layer routes `file:` handles to a small `FileLibrary` (plain
   `java.io.File`). The viewer and "open in system app" (through the existing `FileProvider`) get a `file:`
   branch. The core is unchanged. It already rescans and keeps its data when the same root is opened again, and
   the directory path never changes, so a re-import is that case.
2. **Our own `DocumentsProvider`** over the unpacked directory, so everything keeps seeing SAF tree URIs.
   Rejected: a provider is more code and an exported component to secure, for the same result.

## Design

- **Unpacking** (`platform/ZipLibrary.kt`) runs on `Dispatchers.IO`:
  - It unpacks into `filesDir/zip-library.new/`, then deletes `zip-library/` and renames the new directory in
    its place, so a failed import keeps the old library.
  - It keeps entries whose name ends in `.pdf` (any case) and everything under a `.notae/` folder (the PC
    sidecar). It skips `__MACOSX/` and other hidden entries.
  - It rejects names that would escape the directory (zip slip: `..` segments or absolute paths).
  - Names are read as UTF-8. If that fails, it retries as CP852 (Polish Windows ZIPs).
  - It stops with "not enough space" when the unpacked bytes would leave less than 50 MB free.
  - A ZIP without a single PDF fails with "no PDFs".
- **Routing**: `PlatformLibrary` is `RoutingLibrary(saf, file)`, which picks by the handle's scheme.
  `FileLibrary` implements the same six calls over `File`. It reuses `sha256Hex` and the `PdfRenderer` code that
  `SafLibrary` has. Shared pieces are moved, not copied.
- **Viewer and system app**: `PdfPageSource.open` and `PdfOpener.find` resolve `file:` handles to the file. The
  system app gets a `FileProvider` URI (`files-path zip-library/`) with a read grant.
- **Received files**: `MainActivity.receive` checks the first bytes. `PK\x03\x04` means a ZIP import (with the
  confirmation when a library is open); anything else is read as a set or backup, as today. The manifest adds
  `application/zip` and `application/x-zip-compressed` to the `VIEW` and `SEND` filters.
- **State** is Kotlin-only, in `NotaeViewModel`: `zipImport: ZipImport?` = `Confirm(name, uri)`,
  `Unpacking(name)` or `Failed(message)`. On success it calls `libraryRootPicked(name, handle)`.
- **Address field**: it trims the address and adds `https://` when the scheme is missing. It accepts only
  `http`/`https` and opens through the existing `openUrl`.
- **Copy**, en/pl:
  - Import scores from a ZIP / Importuj nuty z pliku ZIP
  - Choose ZIP file / Wybierz plik ZIP
  - From a link / Z linku
  - Open in browser / Otwórz w przeglądarce
  - Unpacking “%s”… / Rozpakowuję „%s”…
  - Replace your library with the scores in “%s”? / Zastąpić bibliotekę nutami z „%s”?
  - Replace / Zastąp
  - The errors: This file is not a ZIP. / To nie jest plik ZIP. · There are no PDFs in this ZIP. / W tym pliku
    ZIP nie ma plików PDF. · Not enough space to unpack it. / Za mało miejsca, żeby go rozpakować. · The ZIP
    could not be read: %s / Nie udało się odczytać pliku ZIP: %s
  - The setup intro no longer says scores open "in the PDF app you already use", because they open in Notae
    since 0.8.0. It now reads: "Your sheet music, in good order. Pick the folder with your PDFs, or import them
    from a ZIP file." / pl equivalent.

## Tests

- `ZipLibraryTest` (JVM, temp dirs, ZIPs built in the test):
  - PDFs and `.notae/` are kept; `__MACOSX` and other files are skipped.
  - Zip slip is rejected and the old library is kept.
  - CP852 names come out right.
  - "No PDFs" is reported.
  - Re-import replaces the files at the same path.
- `FileLibraryTest`: enumerate, hash and readFile over a temp dir. `RoutingLibrary` picks by scheme.
- `ZipImportSheetTest` (Robolectric):
  - The address gets `https://` and opens the browser.
  - Choose ZIP file launches the picker.
- A confirmation dialog test.
- The emulator end to end: build a ZIP from `schola/`, share it from the Files app, then open a score in the
  viewer and in the system app.
