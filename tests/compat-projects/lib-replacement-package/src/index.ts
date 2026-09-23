/// <reference lib="dom" />
/// <reference lib="dom.iterable" />
const replaced: ReplacedDom = { replaced: true };
const iterable: ReplacedIterable = { iterable: true };

// The bundled lib.dom.d.ts is gone, so its globals are too.
window.localStorage;

export { replaced, iterable };
