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

if command -v rg >/dev/null 2>&1; then
  source_contract_match() { rg -n 'rounded|shadow|backdrop-filter' "$source_file"; }
  generated_contract_match() { rg --pcre2 -n 'box-shadow:(?!none)|backdrop-filter' "$generated_file"; }
  contains_required() { rg -q -- "$1" "$source_file" "$generated_file"; }
else
  source_contract_match() { grep -En 'rounded|shadow|backdrop-filter' "$source_file"; }
  generated_contract_match() {
    grep -En 'box-shadow:' "$generated_file" |
      grep -Ev 'box-shadow:[[:space:]]*none([;}]|$)';
  }
  contains_required() {
    grep -Eq -- "$1" "$source_file" "$generated_file";
  }
fi

if source_contract_match; then
  echo "runtime CSS source contains a rounded, shadowed, or blurred surface" >&2
  exit 1
fi

if generated_contract_match; then
  echo "generated runtime CSS contains a panel/control shadow or backdrop blur" >&2
  exit 1
fi

for required in '--ikasue-canvas' '--ikasue-density-control' 'border-radius:0' 'prefers-reduced-motion'; do
  if ! contains_required "$required"; then
    echo "runtime CSS is missing Ikasue contract token or behavior: $required" >&2
    exit 1
  fi
done
