#!/usr/bin/env bash
#
# Publish the surge-ts crates to crates.io in dependency order.
#
# Dry run (default) — packages and verifies each crate without uploading:
#   scripts/publish-crates.sh
#
# Real publish — uploads each crate, waiting for the registry to index it
# before publishing the crates that depend on it:
#   scripts/publish-crates.sh --execute
#
# crates.io requires every dependency to already be published, so the order
# below is a topological sort of the workspace's internal dependency graph.
# surge-ts-diagnostics-codegen is publish=false (internal dev tooling) and is
# intentionally excluded.
set -euo pipefail

# Leaf crates first, dependents last.
CRATES=(
  surge-ts-types
  surge-ts-syntax
  surge-ts-diagnostics
  surge-ts-config
  surge-ts-checker
  surge-ts
  surge-ts-cli
)

EXECUTE=0
if [[ "${1:-}" == "--execute" ]]; then
  EXECUTE=1
fi

for crate in "${CRATES[@]}"; do
  if [[ "$EXECUTE" == "1" ]]; then
    echo "==> publishing $crate"
    cargo publish -p "$crate"
    # Give crates.io a moment to index so the next crate's dependency resolves.
    echo "    waiting for $crate to index..."
    sleep 20
  else
    echo "==> dry-run $crate"
    cargo publish -p "$crate" --dry-run
  fi
done

echo "done."
