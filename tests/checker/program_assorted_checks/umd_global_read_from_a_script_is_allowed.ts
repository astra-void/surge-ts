// @filename: legacy.d.ts
// No import or export, so the file is a script and the UMD global is in
// scope for it. Whether surge can resolve the name is a separate question —
// this pins only that the module-only diagnostic stays off.
export declare function greet(name: string): string;
export as namespace Legacy;
// @filename: script.ts
const greeting = Legacy.greet("x");
