// @surge-compare: spans
// tsc anchors an object-literal member assignability error on the property
// key, not the property value.
let value: { name: string } = { name: 1 };
