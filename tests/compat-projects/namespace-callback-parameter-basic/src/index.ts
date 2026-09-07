import * as toolkit from 'toolkit';

export function findConfig(searchPath: string): string | undefined {
  return toolkit.findConfigFile(searchPath, (fileName) =>
    toolkit.sys.fileExists(fileName),
  );
}

export function findNamedConfig(searchPath: string): string | undefined {
  return toolkit.findConfigFile(
    searchPath,
    (fileName) => toolkit.sys.fileExists(fileName),
    'tsconfig.json',
  );
}

export function readConfig(path: string): unknown {
  return toolkit.readConfigFile(path, (file) => toolkit.sys.readFile(file))
    .config;
}

export function versionLength(): number {
  return toolkit.version().length;
}

export function callbackParameterIsChecked(searchPath: string): string | undefined {
  return toolkit.findConfigFile(searchPath, (fileName) => fileName.missing);
}
