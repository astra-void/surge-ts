// @surge-args: --diagnosticProfile native
// @surge-expect: none
// @filename: dom.d.ts
// An overload group whose returns disagree because one did not resolve degrades
// rather than committing to the resolved one — `createElement("textarea")` must
// not read as the string overload's `HTMLElement`.
interface TagMap { textarea: { select(): void } }
interface Doc {
make<K extends keyof TagMap>(tag: K): TagMap[K];
make(tag: string): { id: string };
}
declare const doc: Doc;
// @filename: index.ts
doc.make("textarea").select();
