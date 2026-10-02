import { readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { expect, test } from 'vitest';

import { surge, tempProject } from '../harness/cli.ts';
import { workspacePath } from '../harness/suite.ts';

async function runCli(args: string[]): Promise<string> {
	const result = await surge(args, { cwd: tmpdir() });
	expect([0, 2], `surge exited with ${result.exitCode}\n${result.stderr}`).toContain(
		result.exitCode,
	);
	return result.stdout;
}

test('compat_report_does_not_classify_root_causes', async () => {
	const root = tempProject({
		'tsconfig.json':
			'{ "compilerOptions": { "strict": true, "noEmit": true }, "include": ["src/**/*.ts"] }',
		'src/index.ts': `
        import { MissingThing } from "./missing";

        export const value: MissingThing = UnknownGlobal;
        `,
	});
	const project = join(root, 'tsconfig.json');
	const text = await runCli(['--project', project, '--compatReport']);
	const json = await runCli(['--project', project, '--compatReport', '--format', 'json']);

	const reportSource = readFileSync(workspacePath('crates/surge-ts-cli/src/report.rs'), 'utf8');
	const testsStart = reportSource.indexOf('#[cfg(test)]');
	const implementationSource = testsStart === -1 ? reportSource : reportSource.slice(0, testsStart);

	for (const needle of [
		'nodeModulesSourceDiagnostics',
		'nodeModulesJavaScriptSourceDiagnostics',
		'CategorizedCountEntry',
		'candidate',
		'category',
	]) {
		expect(text, `compat report text still contains classifier text: ${needle}`).not.toContain(
			needle,
		);
		expect(json, `compat report json still contains classifier text: ${needle}`).not.toContain(
			needle,
		);
		expect(implementationSource, `report.rs still contains classifier text: ${needle}`).not.toContain(
			needle,
		);
	}
});
