import { existsSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join, resolve } from 'node:path';

export const WORKSPACE_ROOT = resolve(import.meta.dirname, '../..');

export function surgeBinary(): string {
	const fromEnv = process.env.SURGE_TS_BIN;
	if (fromEnv) return resolve(fromEnv);
	return join(WORKSPACE_ROOT, 'target/release/surge');
}

/**
 * The platform-native tsgo executable behind the pinned `typescript` package.
 * Invoking it directly skips a Node startup per fixture.
 */
export function tscBinary(): string {
	const require = createRequire(join(WORKSPACE_ROOT, 'package.json'));
	const typescriptDir = dirname(require.resolve('typescript/package.json'));
	const platformPackage = `@typescript/typescript-${process.platform}-${process.arch}`;
	const nativeRequire = createRequire(join(typescriptDir, 'package.json'));
	const nativeDir = dirname(nativeRequire.resolve(`${platformPackage}/package.json`));
	const exe = join(nativeDir, 'lib', process.platform === 'win32' ? 'tsc.exe' : 'tsc');
	if (!existsSync(exe)) {
		throw new Error(`tsgo executable not found at ${exe}; run pnpm install`);
	}
	return exe;
}

export function tscVersion(): string {
	const require = createRequire(join(WORKSPACE_ROOT, 'package.json'));
	const manifest = JSON.parse(
		readFileSync(require.resolve('typescript/package.json'), 'utf8'),
	) as { version: string };
	return manifest.version;
}
