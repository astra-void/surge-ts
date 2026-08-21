// `Env.Vars` is re-opened by two packages: `@types/pkgruntime` contributes the
// `extends Dict<string>` that gives it an index signature, `pkgapp` contributes
// `MODE`. Resolving through either must see both.
import { appName } from 'pkgapp';

declare const vars: Env.Vars;

export const name = appName();
export const mode = vars.MODE;
export const tz = vars.TZ;
export const fromIndex: number = vars.ANYTHING;
