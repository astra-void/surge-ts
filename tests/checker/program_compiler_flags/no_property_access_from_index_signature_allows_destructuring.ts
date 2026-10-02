// @noPropertyAccessFromIndexSignature: true
interface D { [k: string]: number; } declare const d: D; const { foo } = d; const use = foo;
