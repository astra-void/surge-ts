// @surge-args: --diagnosticProfile native
// @surge-expect: none
// @filename: types/globals.d.ts
// A wildcard pattern resolves any specifier it matches, so the CSS/asset
// declarations app frameworks ship (`declare module "*.css"`) stop the
// importing side from reporting an unresolved module.
declare module "*.css" { const c: string; export default c; }
// @filename: index.ts
import "./app.css";
export const x = 1;
