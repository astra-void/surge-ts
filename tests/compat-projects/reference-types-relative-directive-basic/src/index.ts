// `PKG_RELATIVE_MODE` and `PkgRelativeGlobals` are declared in a file the
// package reaches only through a relative `/// <reference types="./…" />`.
declare const env: PkgRelativeGlobals.Env;

export const mode: 'development' | 'production' = PKG_RELATIVE_MODE;
export const envMode: 'development' | 'production' = env.MODE;
