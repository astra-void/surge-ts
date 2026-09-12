#!/usr/bin/env bash
# Provision the TanStack Query real-project corpus under .local-projects/.
# Usage: scripts/real-projects/provision-tanstack-query.sh [git-ref]
set -euo pipefail

REF="${1:-cdbe8cb00242ec00c33917ecc05ff33afb41ef5a}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DEST="$WORKSPACE_ROOT/.local-projects/tanstack-query"

if [ ! -d "$DEST/.git" ]; then
  git clone --filter=blob:none https://github.com/TanStack/query.git "$DEST"
fi

# GitHub only serves a bare object to `fetch` when the full sha is asked for,
# so fall back to fetching the default branch for a branch/tag/short-sha ref.
git -C "$DEST" fetch --filter=blob:none origin "$REF" \
  || git -C "$DEST" fetch --filter=blob:none origin
git -C "$DEST" checkout --detach "$REF"

# The repo pins pnpm 11 in packageManager; corepack honours that pin. Only the
# packages/ workspace is installed — examples/ and integrations/ pull in Angular,
# Next and Vite toolchains the corpus never checks.
(cd "$DEST" && corepack pnpm install --filter "./packages/**" --ignore-scripts)

# The upstream root tsconfig covers only "*.config.*"; each package carries its
# own. tsconfig.surge.json is the aggregate single-program target the harness
# measures, kept in the repo because the corpus itself is untracked.
cp "$SCRIPT_DIR/targets/tanstack-query.tsconfig.json" "$DEST/tsconfig.surge.json"

echo "provisioned $DEST at $(git -C "$DEST" rev-parse --short HEAD)"
