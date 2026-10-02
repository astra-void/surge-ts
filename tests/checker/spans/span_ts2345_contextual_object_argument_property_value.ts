// @surge-compare: spans
// tsc anchors the object-literal member error on the argument property key.
function takesUser(value: { name: string }): void { } takesUser({ name: 1 });
