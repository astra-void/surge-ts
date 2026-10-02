// tsc applies `noPropertyAccessFromIndexSignature` only to written dotted
// accesses; a destructuring binding from an index-signature type is allowed.
// An interface `get`/`set` accessor pair lowers to a property typed by the
// getter, the shape the DOM lib uses for `Window.location`
// (`get location(): Location; set location(href: string)`).
interface Loc { href: string }
interface Host {
get location(): Loc;
set location(href: string);
}
declare const host: Host;
const href: string = host.location.href;
