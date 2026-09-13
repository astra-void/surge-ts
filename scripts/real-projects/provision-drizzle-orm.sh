#!/usr/bin/env bash
# Provision the Drizzle ORM real-project corpus under .local-projects/.
# Usage: scripts/real-projects/provision-drizzle-orm.sh [git-ref]
set -euo pipefail

REF="${1:-b7862528fd8fc39bc2653a6c18dad7c1f4e68d10}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DEST="$WORKSPACE_ROOT/.local-projects/drizzle-orm"

if [ ! -d "$DEST/.git" ]; then
  git clone --filter=blob:none https://github.com/drizzle-team/drizzle-orm.git "$DEST"
fi

# GitHub only serves a bare object to `fetch` when the full sha is asked for,
# so fall back to fetching the default branch for a branch/tag/short-sha ref.
git -C "$DEST" fetch --filter=blob:none origin "$REF" \
  || git -C "$DEST" fetch --filter=blob:none origin
git -C "$DEST" checkout --detach "$REF"

# The repo pins pnpm 10 in packageManager; corepack honours that pin. Only the
# drizzle-orm package is installed — drizzle-kit, drizzle-seed and
# integration-tests pull in database servers and bundler toolchains the corpus
# never checks. The package's devDependencies carry every driver its sources
# import, so the filtered install is enough to type the whole of src/.
(cd "$DEST" && corepack pnpm install --filter ./drizzle-orm --ignore-scripts)

# The target lives in the package directory, not the repo root: `paths` and
# `typeRoots` resolve relative to the tsconfig that declares them, and upstream's
# own type gate (`cd type-tests && tsc`) runs from there too. It is kept in this
# repo because the corpus itself is untracked.
cp "$SCRIPT_DIR/targets/drizzle-orm.tsconfig.json" "$DEST/drizzle-orm/tsconfig.surge.json"

echo "provisioned $DEST at $(git -C "$DEST" rev-parse --short HEAD)"
