import { existsSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

import { WORKSPACE_ROOT, surgeBinary, tscBinary } from './paths.ts';

/**
 * Refuses to run against a surge binary older than the Rust sources: a stale
 * binary silently measures someone else's checker. An explicit SURGE_TS_BIN is
 * trusted as-is so a pinned build can be tested.
 */
export default function setup(): void {
	tscBinary();
	const binary = surgeBinary();
	if (!existsSync(binary)) {
		throw new Error(
			`surge binary not found at ${binary}; run \`cargo build --release -p surge-ts-cli\` or set SURGE_TS_BIN`,
		);
	}
	if (process.env.SURGE_TS_BIN) return;

	const builtAt = statSync(binary).mtimeMs;
	const newer = newestRustSource(join(WORKSPACE_ROOT, 'crates'));
	if (newer !== null && newer.mtimeMs > builtAt) {
		throw new Error(
			`surge binary ${binary} is older than ${newer.path}; run \`cargo build --release -p surge-ts-cli\``,
		);
	}
}

function newestRustSource(dir: string): { path: string; mtimeMs: number } | null {
	let newest: { path: string; mtimeMs: number } | null = null;
	for (const entry of readdirSync(dir, { withFileTypes: true })) {
		if (entry.name === 'target' || entry.name === 'tests') continue;
		const path = join(dir, entry.name);
		const candidate = entry.isDirectory()
			? newestRustSource(path)
			: /\.(?:rs|toml)$/.test(entry.name)
				? { path, mtimeMs: statSync(path).mtimeMs }
				: null;
		if (candidate !== null && (newest === null || candidate.mtimeMs > newest.mtimeMs)) {
			newest = candidate;
		}
	}
	return newest;
}
