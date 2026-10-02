// An arrow argument whose parameter type is a callable object (an interface
// carrying a call signature, e.g. React's `ForwardRefRenderFunction`) is
// contextually typed by that call signature rather than left implicit-any: `v`
// resolves to `number`, so assigning it to `string` is the only diagnostic.
interface Render { (x: number): void; }
declare function take(fn: Render): void;
take((v) => { const s: string = v; });
