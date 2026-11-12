#!/usr/bin/env bash
# Assembles the project site (https://notae.c2yz.com) for the Pages workflow: README.md becomes the
# home page and PRIVACY.md /privacy/, with the README's images and the layout in .github/pages/.
#
# Usage: pages-site.sh <output-dir>
#
# The site is built by GitHub's Jekyll (kramdown), so the GitHub-only Markdown is adapted here:
# relative links point at the repository, `> [!NOTE]` alerts become a bold label and the Mermaid
# diagram, which nothing renders on the site, is dropped.
set -euo pipefail

out=${1:?usage: $0 <output-dir>}
repo=$(cd "$(dirname "$0")/.." && pwd)
blob=https://github.com/eRgo35/notae/blob/main

mkdir -p "$out/privacy" "$out/docs" "$out/_layouts"
cp -r "$repo/docs/images" "$out/docs/images"
cp "$repo/.github/pages/default.html" "$out/_layouts/default.html"
cat > "$out/_config.yml" <<'YAML'
title: Notae
description: Browse and search your sheet-music PDFs on an Android phone or tablet. No cloud, no account, no network access.
YAML

# Empty front matter keeps Jekyll from also publishing the Markdown source next to the page.
adapt() {
  printf -- "---\n---\n"
  perl -0pe "
    s#\]\((?!https?:|\#|mailto:)([^)]+)\)#]($blob/\$1)#g;
    s#href=\"(?!https?:|\#|mailto:)([^\"]+)\"#href=\"$blob/\$1\"#g;
    s#^(<div align=\"center\"|<details)>#\$1 markdown=\"1\">#mg;
    s#<summary>#<summary markdown=\"span\">#g;
    s#^> \[!(\w)(\w+)\]\n#> **\$1\L\$2\E**\n>\n#mg;
    s#^\`\`\`mermaid\n.*?^\`\`\`\n\n##msg;
  " "$1"
}
adapt "$repo/README.md" > "$out/index.md"
adapt "$repo/PRIVACY.md" > "$out/privacy/index.md"
