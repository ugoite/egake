#!/usr/bin/env bash
set -euo pipefail

# This audit intentionally uses a small, explicit list. It does not treat
# arbitrary names such as "secrets", "credentials", or "dist" in any
# directory as sensitive/generated: those would produce too many false
# positives for source repositories.

repository_root=$(git rev-parse --show-toplevel)
cd "$repository_root"

classify_path() {
    local path=$1

    case "$path" in
        target|target/*|*.rs.bk)
            printf '%s\t%s\n' rust-build "$path"
            ;;
        node_modules|node_modules/*|*/node_modules|*/node_modules/*|.npm|.npm/*|*/.npm|*/.npm/*|.pnpm-store|.pnpm-store/*|*/.pnpm-store|*/.pnpm-store/*)
            printf '%s\t%s\n' node-dependencies "$path"
            ;;
        .deno|.deno/*|*/.deno|*/.deno/*)
            printf '%s\t%s\n' deno-metadata "$path"
            ;;
        dist|dist/*|docsite/dist|docsite/dist/*|docsite/.astro|docsite/.astro/*)
            printf '%s\t%s\n' docs-generated "$path"
            ;;
        coverage|coverage/*|*.profraw|*.tsbuildinfo|.coverage|*/.coverage)
            printf '%s\t%s\n' generated-cache "$path"
            ;;
        .python-version|*/.python-version|.venv|.venv/*|*/.venv|*/.venv/*|.ruff_cache|.ruff_cache/*|*/.ruff_cache|*/.ruff_cache/*|.ty|.ty/*|*/.ty|*/.ty/*|.pytest_cache|.pytest_cache/*|*/.pytest_cache|*/.pytest_cache/*|.mypy_cache|.mypy_cache/*|*/.mypy_cache|*/.mypy_cache/*|__pycache__|__pycache__/*|*/__pycache__|*/__pycache__/*|*.pyc|*.pyo|*.pyd|*.egg-info|*.egg-info/*|*/.egg-info|*/.egg-info/*|pip-wheel-metadata|pip-wheel-metadata/*|*/pip-wheel-metadata|*/pip-wheel-metadata/*|htmlcov|htmlcov/*|*/htmlcov|*/htmlcov/*)
            printf '%s\t%s\n' python-cache "$path"
            ;;
        .env|.env.*|*/.env|*/.env.*)
            case "$path" in
                .env.example|*/.env.example)
                    return 0
                    ;;
            esac
            printf '%s\t%s\n' local-secret "$path"
            ;;
        .idea|.idea/*|*/.idea|*/.idea/*|.vscode|.vscode/*|*/.vscode|*/.vscode/*|*.iml|*/.iml|.DS_Store|*/.DS_Store|._*|*/._*|.AppleDouble|*/.AppleDouble|.LSOverride|*/.LSOverride|*.swp|*.swo|*~)
            printf '%s\t%s\n' editor-os "$path"
            ;;
        npm-debug.log*|*/npm-debug.log*|yarn-debug.log*|*/yarn-debug.log*|yarn-error.log*|*/yarn-error.log*|pnpm-debug.log*|*/pnpm-debug.log*)
            printf '%s\t%s\n' package-manager-log "$path"
            ;;
    esac
}

print_patterns() {
    cat <<'EOF'
Explicit audit patterns:
  rust-build       target/**, *.rs.bk
  node-dependencies **/node_modules/**, **/.npm/**, **/.pnpm-store/**
  deno-metadata    **/.deno/**
  docs-generated   /dist/**, /docsite/dist/**, /docsite/.astro/**
  generated-cache  /coverage/**, *.profraw, *.tsbuildinfo, **/.coverage
  python-cache     **/.python-version, **/.venv/**, **/.ruff_cache/**,
                   **/.ty/**, **/.pytest_cache/**, **/.mypy_cache/**,
                   **/__pycache__/**, *.pyc, *.pyo, *.pyd, *.egg-info/**,
                   **/pip-wheel-metadata/**, **/htmlcov/**
  local-secret     **/.env and **/.env.* except **/.env.example
  editor-os        **/.idea/**, **/.vscode/**, **/.DS_Store, ._*,
                   .AppleDouble, .LSOverride, *.iml, *.swp, *.swo, *~
  package-manager-log npm-debug.log*, yarn-debug.log*, yarn-error.log*,
                      pnpm-debug.log*
EOF
}

collect_current_matches() {
    git ls-files -z | while IFS= read -r -d '' path; do
        classify_path "$path"
    done | sort -u
}

collect_history_matches() {
    # rev-list --objects --all covers paths in every object reachable from
    # every local ref, including files removed from the current tree.
    git rev-list --objects --all -z | while IFS= read -r -d '' object_and_path; do
        case "$object_and_path" in
            *' '*) classify_path "${object_and_path#* }" ;;
        esac
    done | sort -u
}

print_matches() {
    local title=$1
    local matches=$2

    printf '\n%s\n' "$title"
    if [ -n "$matches" ]; then
        printf '%s\n' "$matches"
    else
        printf '%s\n' '(none)'
    fi
}

print_patterns

current_matches=$(collect_current_matches)
history_matches=$(collect_history_matches)

print_matches 'Currently tracked matches (git ls-files):' "$current_matches"
print_matches 'Matches in reachable history (git rev-list --objects --all):' "$history_matches"

failure=0

if [ -n "$current_matches" ]; then
    printf '\nERROR: tracked generated, secret, cache, or local-only paths were found.\n' >&2
    failure=1
fi

if [ -n "$history_matches" ]; then
    printf '\nERROR: reachable history contains paths covered by the explicit audit patterns.\n' >&2
    printf '%s\n' 'Do not rewrite history in this worktree; have the integration orchestrator verify the exact target before purging.' >&2
    failure=1
fi

for path in \
    src/example.rs \
    src/example.ts \
    src/example.tsx \
    src/example.js \
    src/example.jsx \
    src/example.py \
    docs/example.md \
    Cargo.lock \
    deno.lock \
    package-lock.json
do
    if git check-ignore --no-index -q "$path"; then
        printf '\nERROR: expected trackable path is ignored: %s\n' "$path" >&2
        failure=1
    fi
done

exit "$failure"
