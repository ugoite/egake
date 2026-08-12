#!/usr/bin/env bash
set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "$repository_root"

source_file="crates/egake-cli/assets/runtime.css.src"
generated_file="crates/egake-cli/assets/runtime.css"
temporary_directory=$(mktemp -d "${TMPDIR:-/tmp}/egake-runtime-css.XXXXXX")
trap 'rm -rf "$temporary_directory"' EXIT

deno run --frozen --node-modules-dir=auto -A npm:@tailwindcss/cli@4.3.0 \
  --input "$source_file" \
  --output "$temporary_directory/runtime.css" \
  --minify >/dev/null

if ! cmp -s "$temporary_directory/runtime.css" "$generated_file"; then
  echo "runtime.css is out of date; run 'mise run ui:build'" >&2
  exit 1
fi

if rg -n 'rounded|shadow|backdrop-filter' "$source_file"; then
  echo "runtime CSS source contains a rounded, shadowed, or blurred surface" >&2
  exit 1
fi

if rg --pcre2 -n 'box-shadow:(?!none)|backdrop-filter' "$generated_file"; then
  echo "generated runtime CSS contains a panel/control shadow or backdrop blur" >&2
  exit 1
fi

for required in '--ikasue-canvas' '--ikasue-density-control' 'border-radius:0' 'prefers-reduced-motion'; do
  if ! rg -q -- "$required" "$source_file" "$generated_file"; then
    echo "runtime CSS is missing Ikasue contract token or behavior: $required" >&2
    exit 1
  fi
done
