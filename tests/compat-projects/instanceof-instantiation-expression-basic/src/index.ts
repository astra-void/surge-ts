class C<T> {}
declare const x: unknown;
if (x instanceof C<string>) {}
if (x instanceof (C<string>)) {}
if (x instanceof C) {}
const y = C<string>;
if (x instanceof y) {}
export {};
