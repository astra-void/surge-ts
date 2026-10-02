// Every option `tsc --help --all` (7.0.2) lists that can appear in a tsconfig's
// compilerOptions. Fixture directives are matched case-insensitively, as in the
// TypeScript test harness, and written under the canonical spelling.
const COMPILER_OPTIONS = [
	'allowArbitraryExtensions',
	'allowImportingTsExtensions',
	'allowJs',
	'allowSyntheticDefaultImports',
	'allowUmdGlobalAccess',
	'allowUnreachableCode',
	'allowUnusedLabels',
	'alwaysStrict',
	'baseUrl',
	'checkJs',
	'customConditions',
	'declaration',
	'declarationDir',
	'declarationMap',
	'downlevelIteration',
	'emitBOM',
	'emitDeclarationOnly',
	'emitDecoratorMetadata',
	'erasableSyntaxOnly',
	'esModuleInterop',
	'exactOptionalPropertyTypes',
	'experimentalDecorators',
	'forceConsistentCasingInFileNames',
	'importHelpers',
	'inlineSourceMap',
	'inlineSources',
	'isolatedDeclarations',
	'isolatedModules',
	'jsx',
	'jsxFactory',
	'jsxFragmentFactory',
	'jsxImportSource',
	'lib',
	'libReplacement',
	'mapRoot',
	'maxNodeModuleJsDepth',
	'module',
	'moduleDetection',
	'moduleResolution',
	'moduleSuffixes',
	'newLine',
	'noCheck',
	'noEmit',
	'noEmitHelpers',
	'noEmitOnError',
	'noErrorTruncation',
	'noFallthroughCasesInSwitch',
	'noImplicitAny',
	'noImplicitOverride',
	'noImplicitReturns',
	'noImplicitThis',
	'noLib',
	'noPropertyAccessFromIndexSignature',
	'noResolve',
	'noUncheckedIndexedAccess',
	'noUncheckedSideEffectImports',
	'noUnusedLocals',
	'noUnusedParameters',
	'outDir',
	'outFile',
	'paths',
	'preserveConstEnums',
	'preserveSymlinks',
	'reactNamespace',
	'removeComments',
	'resolveJsonModule',
	'resolvePackageJsonExports',
	'resolvePackageJsonImports',
	'rewriteRelativeImportExtensions',
	'rootDir',
	'rootDirs',
	'skipDefaultLibCheck',
	'skipLibCheck',
	'sourceMap',
	'sourceRoot',
	'strict',
	'strictBindCallApply',
	'strictBuiltinIteratorReturn',
	'strictFunctionTypes',
	'strictNullChecks',
	'strictPropertyInitialization',
	'stripInternal',
	'target',
	'typeRoots',
	'types',
	'useDefineForClassFields',
	'useUnknownInCatchVariables',
	'verbatimModuleSyntax',
] as const;

const LIST_OPTIONS = new Set([
	'customConditions',
	'lib',
	'moduleSuffixes',
	'rootDirs',
	'typeRoots',
	'types',
]);

// Directives the TypeScript test harness understands that are not compiler
// options; upstream fixtures carry them and they have no tsconfig meaning.
const HARNESS_ONLY_DIRECTIVES = new Set(
	[
		'allowNonTsExtensions',
		'baselineFile',
		'captureSuggestions',
		'currentDirectory',
		'fullEmitPaths',
		'includeBuiltFile',
		'libFiles',
		'link',
		'noImplicitReferences',
		'noTypesAndSymbols',
		'reportDiagnostics',
		'symlink',
		'traceResolution',
	].map((name) => name.toLowerCase()),
);

const CANONICAL = new Map<string, string>(
	COMPILER_OPTIONS.map((name) => [name.toLowerCase(), name]),
);

export function canonicalCompilerOption(name: string): string | undefined {
	return CANONICAL.get(name.toLowerCase());
}

export function isHarnessOnlyDirective(name: string): boolean {
	return HARNESS_ONLY_DIRECTIVES.has(name.toLowerCase());
}

export function parseCompilerOptionValue(name: string, raw: string): unknown {
	const value = raw.trim();
	if (value.startsWith('{') || value.startsWith('[')) {
		return JSON.parse(value);
	}
	if (LIST_OPTIONS.has(name)) {
		return value
			.split(',')
			.map((entry) => entry.trim())
			.filter((entry) => entry.length > 0);
	}
	// Upstream fixtures list several values to run a case once per variant; the
	// first variant is the one checked here.
	const first = value.split(',')[0].trim();
	if (first === 'true') return true;
	if (first === 'false') return false;
	if (/^\d+$/.test(first)) return Number(first);
	return first;
}
