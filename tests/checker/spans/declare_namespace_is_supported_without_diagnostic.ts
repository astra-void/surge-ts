// @surge-args: --diagnosticProfile native
// @surge-expect: none
// @filename: example.d.ts
// Namespaces are parsed so qualified members resolve (`JSX.IntrinsicElements`);
// an empty namespace emits nothing, matching tsc.
declare namespace N {}
