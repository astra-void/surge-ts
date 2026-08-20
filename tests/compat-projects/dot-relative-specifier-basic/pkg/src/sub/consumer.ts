// `..` is a relative specifier, so it must resolve to pkg/src/index.ts —
// never through package resolution to the package's own `types` entry.
import { fromSourceIndex } from '..';

export const value: number = fromSourceIndex;
