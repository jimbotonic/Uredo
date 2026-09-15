#!/usr/bin/env bash
# Build docs/MANUAL.pdf from docs/MANUAL.md.
#
# Reproducible and committed, so the PDF is a build product rather than an artefact nobody can
# regenerate. Needs pandoc and pdflatex, plus TeX Live's Source Serif/Sans/Code Pro packages.
#
# pdfTeX rather than LuaTeX or XeTeX on purpose: the manual's only non-ASCII characters are `§`,
# the em dash, the ellipsis and the middle dot, all of which T1 has, so the Unicode engines buy
# nothing here — and this machine's luaotfload is incomplete (`luaotfload-main.lua` is missing),
# which makes lualatex fall back to OT1 and fail. Pin the engine that works everywhere.
#
#   docs/pdf/build.sh            -> docs/MANUAL.pdf
#   docs/pdf/build.sh out.pdf    -> out.pdf
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
docs="$(dirname "$here")"
out="${1:-$docs/MANUAL.pdf}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

for tool in pandoc pdflatex; do
  command -v "$tool" >/dev/null || { echo "$tool is not on the path" >&2; exit 1; }
done

# The manual's own H1 becomes the title page, its hand-written contents list becomes the table of
# contents, and its `---` rules become page breaks; all three are dropped here rather than in the
# markdown, which has to stay readable on its own. Code fences are tracked so a `---` or a `##`
# inside a listing survives.
awk '
  /^```/           { fence = !fence; print; next }
  fence            { print; next }
  NR == 1 && /^# / { next }
  /^## Contents$/  { skip = 1; next }
  skip && /^---$/  { skip = 0; next }
  skip             { next }
  /^---$/          { next }
                   { print }
' "$docs/MANUAL.md" > "$work/manual.md"

# The date the manual was last changed, not the date of the build, so two builds of one source
# produce the same document.
docdate="$(cd "$docs" && git log -1 --format=%cs -- MANUAL.md 2>/dev/null || true)"
[ -n "$docdate" ] || docdate="$(date +%F)"
{ printf '\\newcommand{\\docdate}{%s}\n' "$docdate"; cat "$here/manual-head.tex"; } > "$work/head.tex"

# pdfTeX stamps the build time into the PDF unless told otherwise, which makes every rebuild a
# diff even when nothing changed. Pin both clocks to the manual's own last-changed date so two
# builds of one source are byte-identical.
export SOURCE_DATE_EPOCH="$(date -u -d "$docdate" +%s)"
export FORCE_SOURCE_DATE=1

pandoc "$work/manual.md" \
  --from=markdown+smart \
  --shift-heading-level-by=-1 \
  --pdf-engine=pdflatex \
  --syntax-definition="$here/uredo.xml" \
  --syntax-highlighting="$here/uredo.theme" \
  --include-in-header="$work/head.tex" \
  --toc --toc-depth=1 \
  --metadata title="The Uredo manual" \
  --metadata date="$docdate" \
  --metadata lang=en-GB \
  --variable documentclass=article \
  --variable classoption=11pt \
  --variable papersize=a4 \
  --variable geometry:"a4paper,inner=28mm,outer=28mm,top=25mm,bottom=25mm,headsep=8mm" \
  --variable colorlinks=true \
  --variable linkcolor=uredoaccent \
  --variable urlcolor=uredoaccent \
  --variable toccolor=uredodeep \
  --variable linestretch=1.06 \
  --variable numbersections=false \
  --output="$out"

echo "$out"
