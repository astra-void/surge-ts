#!/usr/bin/env bash
# Provision the ts-pattern real-project corpus under .local-projects/.
# Usage: scripts/real-projects/provision-ts-pattern.sh [git-ref]
set -euo pipefail

REF="${1:-c92ca435c7e1827e0fd55c539080ef1bfd6fe3f0}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DEST="$WORKSPACE_ROOT/.local-projects/ts-pattern"

if [ ! -d "$DEST/.git" ]; then
  git clone --filter=blob:none https://github.com/gvergnaud/ts-pattern.git "$DEST"
fi

# GitHub only serves a bare object to `fetch` when the full sha is asked for,
# so fall back to fetching the default branch for a branch/tag/short-sha ref.
git -C "$DEST" fetch --filter=blob:none origin "$REF" \
  || git -C "$DEST" fetch --filter=blob:none origin
git -C "$DEST" checkout --detach "$REF"

# npm, not pnpm: the repo ships package-lock.json and no packageManager pin.
# Only the test suite needs dependencies at all (@types/jest); src/ has none.
(cd "$DEST" && npm ci --ignore-scripts)

# The upstream root tsconfig covers src/ only and emits declarations; tests/
# carries its own with `downlevelIteration`, which TypeScript 7 removed
# (TS5102). tsconfig.surge.json is the aggregate single-program target the
# harness measures, kept in the repo because the corpus itself is untracked.
cp "$SCRIPT_DIR/targets/ts-pattern.tsconfig.json" "$DEST/tsconfig.surge.json"

echo "provisioned $DEST at $(git -C "$DEST" rev-parse --short HEAD)"
