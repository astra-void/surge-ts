import { existsSync } from 'node:fs';
import { availableParallelism } from 'node:os';
import { resolve } from 'node:path';

import { defineConfig } from 'vitest/config';

const RELEASE_SURGE = resolve(import.meta.dirname, 'target/release/surge');

// Fixture tests spend their time in child processes (surge and tsc), so a few
// workers each running several fixtures concurrently keep the cores busy.
export default defineConfig({
	test: {
		maxWorkers: Math.max(1, Math.min(4, Math.floor(availableParallelism() / 2))),
		maxConcurrency: 4,
		testTimeout: 120_000,
		projects: [
			{
				extends: true,
				test: {
					name: 'surge',
					include: ['tests/**/*.test.ts'],
					exclude: ['**/node_modules/**', 'tests/compat-projects/**'],
					globalSetup: ['tests/harness/global-setup.ts'],
				},
			},
			{
				// The scripts' own harness tests need no release build. The few
				// that run surge fall back to a synchronous debug `cargo build`
				// when SURGE_TS_BIN is unset, which blocks a worker for minutes, so
				// an existing release binary is used instead.
				extends: true,
				test: {
					name: 'scripts',
					include: ['scripts/**/*.test.ts'],
					env:
						process.env.SURGE_TS_BIN === undefined && existsSync(RELEASE_SURGE)
							? { SURGE_TS_BIN: RELEASE_SURGE }
							: {},
				},
			},
		],
	},
});
