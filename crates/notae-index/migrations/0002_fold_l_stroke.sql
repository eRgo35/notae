-- Notae index schema, version 2.
--
-- Version 1 wrote the stored text into `notes_fts` unchanged. FTS5
-- `unicode61 remove_diacritics 2` cannot fold U+0142/U+0141 (ł/Ł: no canonical
-- decomposition), so a query typed without the stroke found nothing. From
-- version 2 every writer folds ł to l and Ł to L before the text reaches the
-- tokenizer (`crate::fold::fold_l_stroke`) and every query is folded the same
-- way. This migration brings the rows written by version 1 up to the same form,
-- in place, inside the migration transaction.
--
-- Only the derived `notes_fts` table changes; the canonical `notes` rows keep
-- the original spelling, and the sidecar manifest (which is built from `notes`)
-- is unaffected. Rows without ł/Ł are not rewritten.
--
-- The upgrade is forward-only: builds that only know version 1 refuse a
-- version 2 database.
--
-- A released migration is frozen. The mapping below is a copy of
-- `fold_l_stroke`; `a_version_1_index_is_folded_in_place_when_it_is_opened` in
-- `tests/store_and_search.rs` checks that the two agree.

UPDATE notes_fts
SET title    = replace(replace(title,    'ł', 'l'), 'Ł', 'L'),
    composer = replace(replace(composer, 'ł', 'l'), 'Ł', 'L'),
    tags     = replace(replace(tags,     'ł', 'l'), 'Ł', 'L'),
    filename = replace(replace(filename, 'ł', 'l'), 'Ł', 'L'),
    ocr_text = replace(replace(ocr_text, 'ł', 'l'), 'Ł', 'L')
WHERE instr(coalesce(title, '') || coalesce(composer, '') || coalesce(tags, '')
            || coalesce(filename, '') || coalesce(ocr_text, ''), 'ł') > 0
   OR instr(coalesce(title, '') || coalesce(composer, '') || coalesce(tags, '')
            || coalesce(filename, '') || coalesce(ocr_text, ''), 'Ł') > 0;
