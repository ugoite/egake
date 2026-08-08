#!/usr/bin/env bash
set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "$repository_root"

path_matches=$(git ls-files | rg -i 'ikashita' || true)

text_matches=$(git grep --cached -I -n -i -e 'ikashita' -- . \
    ':(exclude)scripts/check-brand.sh' || true)

if [ -n "$path_matches" ] || [ -n "$text_matches" ]; then
    printf 'egake brand check failed: old product identity remains.\n' >&2
    if [ -n "$path_matches" ]; then
        printf '\nTracked paths:\n%s\n' "$path_matches" >&2
    fi
    if [ -n "$text_matches" ]; then
        printf '\nTracked text:\n%s\n' "$text_matches" >&2
    fi
    exit 1
fi

printf 'egake brand check passed.\n'
