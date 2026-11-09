# 0003. In-app PDF viewer, notes and drawing overlays

- **Status:** Accepted (2026-11-09; maintainer decision after tablet testing of 0.6.1)
- **Issues:** #46 (viewer), #47 (notes), #48 (drawings), #51 (sharing a set); parent #45
- **Supersedes:** "Not planned: viewing or annotating PDFs inside Notae" in `ROADMAP.md`, and the README statements that Notae hands every PDF to another viewer and has "no in-app PDF viewing or annotation, by design". The README changes when #46 ships.

## Context

Until 0.6.1, Notae opened every score in another app (`ACTION_VIEW`, usually Samsung Notes). On the tablet, leaving Notae for each score breaks the flow through a set. Notes and markings can't belong to a set either, because the other app knows nothing about sets.

## Decision

1. **Notae has its own viewer** (#46), rendered with `PdfRenderer`. It is the default on new installs and after updating from 0.6.x, with a one-time hint after the update. Settings keeps the old behaviour as an option: open scores in the system's default app.
2. **The library stays read-only.** Score notes, set notes and drawings never change a PDF. They live in Notae's private storage, keyed by the score's content key (SHA-256), and are drawn over the page.
3. **Notes at two levels** (#47): a **score note**, shown wherever the score opens, and a **set note** on one occurrence of the score in a set, shown after the score note when opened from that set. The set-wide field is renamed **set description**; Text notes are unchanged.
4. **Drawings belong to one occurrence of a score in a set** (#48), like set notes. A score-wide drawing layer is not part of this decision.
5. **Sharing a set** (#51) can include its set notes and, once #48 lands, its drawings. Score notes are never shared, but they are in the user's own Export sets backup.

## Options considered

- **Keep handing PDFs to another app** (0.6.1): no viewer to maintain, but no notes or drawings per set, and the user leaves Notae for every score. Kept as the Settings option.
- **Write annotations into a copy of the PDF**: other viewers could show them, but copies would double the library, drift from the original and break file identity. Rejected.
- **Overlays in Notae's storage**: the PDF and its identity stay untouched, and overlays can be per set and shared. Chosen.

## Consequences

- Notae now has to keep up a viewer: paging, zoom and a screen that stays on, on phones and tablets.
- The sets file and the export format get new fields (set notes on set items, score notes, drawings), with a schema version bump and migration.
- If a score's PDF changes (new content key), its notes and drawings are kept, but the drawings may no longer line up and the user is warned.
- No network access is added: sharing uses Android's share sheet.
