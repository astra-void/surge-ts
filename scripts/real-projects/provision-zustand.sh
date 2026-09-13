#!/usr/bin/env bash
# Provision the zustand real-project corpus under .local-projects/.
# Usage: scripts/real-projects/provision-zustand.sh [git-ref]
set -euo pipefail

REF="${1:-2115efb9e270e73ad1d3472dfe0e0c7b8c6abcd4}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DEST="$WORKSPACE_ROOT/.local-projects/zustand"

if [ ! -d "$DEST/.git" ]; then
  git clone --filter=blob:none https://github.com/pmndrs/zustand.git "$DEST"
fi

# GitHub only serves a bare object to `fetch` when the full sha is asked for,
# so fall back to fetching the default branch for a branch/tag/short-sha ref.
git -C "$DEST" fetch --filter=blob:none origin "$REF" \
  || git -C "$DEST" fetch --filter=blob:none origin --tags
git -C "$DEST" checkout --detach "$REF"

# The repo pins pnpm 11 in packageManager; corepack honours that pin. Every
# dependency the corpus types (react, immer, redux, use-sync-external-store,
# @redux-devtools/extension, @testing-library/*) is a devDependency of the one
# package, so a plain install is enough.
(cd "$DEST" && corepack pnpm install --ignore-scripts)

# No target file: upstream's root tsconfig.json *is* the corpus. `pnpm
# test:types` runs `tsc --noEmit` against it, it covers src/ and tests/ in one
# program, and the pinned TypeScript 7.0.2 oracle reports zero diagnostics on
# it — so the checkout is measured as it ships.
echo "provisioned $DEST at $(git -C "$DEST" describe --tags --always HEAD)"
