# Notae privacy policy

Effective date: 12 November 2026

This is the privacy policy of Notae, a score library app for Android developed by Michał Czyż as an open-source project (<https://github.com/eRgo35/notae>).

## Summary

Notae does not collect, send or sell any personal data. It has no accounts, no ads, no analytics and no crash reporting, and it cannot connect to the internet. Everything Notae stores stays on your device, under your control. No data reaches the developer.

## Permissions

Notae does not request the internet permission, and no permission that you have to approve: no storage, microphone, camera, location or contacts access. The only permission in the app is an internal one, added by Android's support libraries, that keeps the app's own messages private to the app.

## Your PDF folder

You choose your folder of PDF scores with Android's folder picker. It can be on internal storage, an SD card or any other location the picker offers. Android gives Notae **read-only** access to that folder only. Notae reads the PDFs to build its index, but never changes, moves, renames or deletes them, and never writes into the folder. The only exception is if you pick that folder as the place to save an **Export sets** file.

If you choose a folder from a cloud storage app, that app, not Notae, may download the files when Notae reads them.

## Data stored on your device

Notae keeps the following in its private app storage, which other apps cannot read:

- **the folder you chose:** its name and Android's read-only access to it;
- **an index of your scores:** file paths and names, sizes and dates, a fingerprint (SHA-256) of each file, numbers, titles, composers and tags, including your edits to them (Notae never writes these edits into the PDFs), and the text already embedded in the PDFs (on Android 15 and later);
- **a thumbnail** of each score's first page;
- **your favorites, recently opened scores, sets** (with their descriptions, text items and set notes) **and score notes**;
- **a shared set you received**, until you accept or decline it;
- **your settings:** display preferences, language, and the section and score last shown.

Notae updates this data when you scan your folder, edit something, or open a score.

## Retention and deletion

This data stays on your device until you delete it. Clear Notae's storage in Android's settings, or uninstall the app, and all of it is removed. Your PDF files are not affected. The developer has no copy of your data, so there is nothing the developer could access, correct or delete on your behalf. Files you exported or shared, and Android backups, are not removed by uninstalling Notae.

## Android backup

Notae uses Android's standard app backup. If backup is turned on for your device (for example Google One backup), Android may include Notae's app data in it, and in a transfer to a new device. This is done by Android under your Google account's settings, not by Notae, and you can turn it off in Android's backup settings. After a restore, you choose your PDF folder again.

## When you hand something to another app

Data leaves Notae only when you act. The app that receives it then handles it under its own privacy policy.

- **Opening a score** gives another app temporary read-only access to that one PDF: Samsung Notes if it is installed, otherwise the app you pick in Android's chooser. That app may keep its own copy.
- **Voice search** opens your device's speech recognition app. Notae does not record audio. The speech app may process your voice, possibly online, and gives Notae only the recognised text.
- **Sharing a set** sends a `.notae-set` file and a plain-text list through Android's share sheet to the app you pick. The file contains the set's name, date, description, internal id and last-change time, its text items, and for each score its number, title, composer, path in your library and fingerprint. With **Share set with notes** it also contains your set notes, which then also appear in the text list. It never contains your PDFs, score notes or favorites.
- **Export sets** saves a file to a location you pick. It contains all your sets (with set notes), favorites and score notes, including each score's library path and fingerprint. It does not contain your PDFs, index, thumbnails, recent scores or settings.
- **Importing** a shared set or a backup, from Settings, "Open with" or "Share to Notae", reads only that one file and saves its sets, favorites and notes in Notae's app storage.

## Security

Notae's data is protected by Android's app sandbox: other apps cannot read it. Because Notae has no network access, it never sends your data anywhere, and the only file it offers to other apps is the set file you choose to share.

## Google Play

If you install Notae from Google Play, Google's privacy policy applies to the download. The developer sees only the anonymous, aggregated statistics that Google Play Console provides.

## Children

Notae is not directed at children and collects no data from anyone.

## Changes

Changes to this policy are published in this file with a new effective date. The full history is in the project's Git repository.

## Contact

Questions about this policy: email <mike@c2yz.com>, or open an issue at <https://github.com/eRgo35/notae/issues> (issues are public).
