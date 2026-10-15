## Agent skills

### Issue tracker

Issues live as GitHub Issues, managed with `gh`. See `docs/agents/issue-tracker.md`.

### Triage labels

Use `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, and `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Use the single-context root `GLOSSARY.md` and `docs/adr/` layout. See `docs/agents/domain.md`.

## Engineering standards

Use the workspace-pinned Rust 1.98.1 toolchain. Run the local quality gates and Android arm64 compile command documented in `docs/engineering-standards.md`; CI runs the same gates. Production code must avoid unchecked `unwrap`/`expect`; the test suite may use them for fixtures and assertions. Keep `unsafe` forbidden except for the Android entry-point shim's documented FFI boundary.

Use Conventional Commits: `<type>(optional-scope): imperative summary` (for example, `fix(search): preserve result panel height`). No mandatory Git hook is installed.
