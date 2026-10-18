# Security policy

## Supported versions

Notae is a small, maintainer-run project. Security fixes land on `main` and are
released as a new patch version; older releases are not patched.

| Version | Supported |
| --- | --- |
| Latest release ([GitHub Releases](https://github.com/eRgo35/notae/releases/latest)) | Yes |
| Anything older, and unreleased `main` snapshots | No |

## Reporting a vulnerability

**Please do not open a public issue for a security problem.**

Report it privately with a GitHub Security Advisory:

1. Open the [Security tab](https://github.com/eRgo35/notae/security) of the
   repository.
2. Choose **Report a vulnerability** (see GitHub's guide to
   [privately reporting a vulnerability](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability)).
3. Describe the problem, the affected version or commit, and the smallest steps
   or input that reproduce it. Attach a proof of concept if you have one.

If the Security tab offers no private reporting option for you, open a public
issue that says only that you need a private channel for a security report and
**contains no details**; the maintainer will arrange one.

You can expect an acknowledgement within a few days and a status update when
the problem has been reproduced. This is a volunteer project, so there is no
formal response-time guarantee. Reporters are credited in the advisory unless
they ask not to be.

## Scope

Notae is a local application: the Android app (`org.notae.android`), the Linux
indexer `notae-index`, the sidecar format they exchange, and the release
pipeline that signs the APK. Notae has no server and no network service.

In scope:

- Memory-safety or logic bugs reachable from untrusted input: a malicious PDF,
  a crafted `.notae/manifest.json` or `index.sqlite`, or a hostile file name.
- Anything that lets Notae read, modify, move or delete files outside the
  folder the user granted, or modify the user's PDFs at all (Notae is designed
  never to).
- Data leaving the device. The app declares no permissions, including no
  `INTERNET` permission; a way to make it transmit data would be a
  vulnerability.
- Weaknesses in how releases are built or signed, and ways to publish an APK
  that passes the documented verification without being signed by the release
  key.

Out of scope:

- The behaviour of third-party apps Notae hands PDFs to (your PDF viewer) or of
  Android's speech recognizer used by voice search, which may use network
  services independently of Notae.
- Attacks that need an already rooted or compromised device, physical access to
  an unlocked tablet, or a modified build of the app.
- Vulnerabilities in dependencies that do not affect Notae; please report those
  upstream. The weekly `cargo deny` audit tracks RustSec advisories.
- Missing TalkBack support, which is a known accessibility limitation and not a
  security issue.

## Verifying a release

Releases are built in GitHub Actions from a `vX.Y.Z` tag and signed with a
dedicated release key. The signing certificate's SHA-256 fingerprint is public:

```text
BE:27:CD:78:17:66:C5:C2:8A:9D:B1:06:C0:7B:05:1C:EE:E4:E0:00:31:05:96:45:95:E2:A8:48:F3:71:F6:01
```

Check an APK with `sha256sum -c <apk>.sha256` and
`apksigner verify --print-certs <apk>`. The release procedure, and what the
workflow verifies before publishing, is in
[`docs/android-release.md`](docs/android-release.md).

## Signing key handling

- The release keystore and its passwords exist only in GitHub Actions secrets
  (`NOTAE_KEYSTORE_BASE64`, `NOTAE_KEYSTORE_PASSWORD`, `NOTAE_KEY_ALIAS`,
  `NOTAE_KEY_PASSWORD`) and in the maintainer's offline backups. They are never
  committed to the repository, and the decoded keystores are deleted at the end
  of the release job.
- The release job is the only job with `contents: write`, and it refuses to
  run unless the tag is on `main` and equals the Cargo version. The build never
  falls back to the Android debug key.
- The pinned certificate fingerprint is checked against the built APK, so a
  wrong or substituted secret cannot publish a release.
- If you believe the key has leaked, treat it as a vulnerability and report it
  privately as described above. Rotating the key requires users to uninstall
  and reinstall, so it is done only when necessary.
- Contributors must never commit keystores, passwords, tokens or other
  secrets; see [`CONTRIBUTING.md`](CONTRIBUTING.md).
