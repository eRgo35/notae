## Summary

<!-- What does this change and why? Link the issue: "Fixes #N" or "Refs #N". -->

## How was it verified?

<!-- Commands you ran, emulator or device checks, screenshots (emulator captures must be labelled as such). -->
<!-- Say plainly what you could NOT verify, e.g. "not tried on a physical tablet". -->

## Checklist

Quality gates (see [CONTRIBUTING.md](https://github.com/eRgo35/notae/blob/main/CONTRIBUTING.md) and `docs/engineering-standards.md`):

- [ ] `scripts/check.sh full` passed locally
- [ ] or only `scripts/check.sh fast` (say why `full` was not run)

Change hygiene:

- [ ] Behaviour changes have tests (behavioural UI tests for Android UI changes) and bug fixes include a regression test
- [ ] Commits follow Conventional Commits
- [ ] `CHANGELOG.md` (`[Unreleased]`) and documentation updated for user-visible changes
- [ ] No private data, PDFs from a real library, `Nutki/` content, keystores, tokens or other secrets are included
- [ ] No new network access, telemetry or permissions (or the change is discussed in the linked issue)
