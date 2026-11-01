/**
 * The Android `versionCode` for a `MAJOR.MINOR.PATCH` version: `(1 << 24) | major << 16 |
 * minor << 8 | patch`, the formula releases up to 0.3.0 used, so codes keep rising across the
 * switch to Gradle (0.2.2 is 16777730).
 *
 * Shared by the app's build script (through buildSrc) and its unit tests.
 */
fun versionCodeFor(version: String): Int {
    val parts = version.split('.').map { it.toIntOrNull() }
    require(parts.size == 3 && parts.all { it != null && it in 0..255 }) {
        "version $version is not MAJOR.MINOR.PATCH with parts 0..255"
    }
    val (major, minor, patch) = parts.map { it ?: 0 }
    return (1 shl 24) or (major shl 16) or (minor shl 8) or patch
}

/** The `version` of the `[workspace.package]` table in the workspace `Cargo.toml` text. */
fun workspaceVersion(cargoToml: String): String {
    val table = cargoToml.substringAfter("[workspace.package]", "").substringBefore("\n[")
    val match = Regex("""(?m)^version\s*=\s*"([^"]+)"""").find(table)
    return requireNotNull(match) { "no version in [workspace.package] of Cargo.toml" }.groupValues[1]
}
