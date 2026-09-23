# unreachable-code-basic

Under an explicit `allowUnreachableCode: false`, the first unreachable potentially-executable statement of a list and the consecutive ones after it are one TS7027, as the binder's reachability decides it: jumps, literal `true`/`false` loop and branch conditions, `try`/`finally`, and non-completing IIFEs. Function declarations, types, const enums and `var` without an initializer are exempt.
