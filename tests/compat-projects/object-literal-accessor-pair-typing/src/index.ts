// An unannotated set accessor parameter is the getter's return type, whichever
// order the pair is written in and however the names are spelled.
const paired = {
  get label() { return "a"; },
  set label(value) { const upper: string = value.toUpperCase(); },
};
const quoted = {
  get 'size'() { return 1; },
  set size(value) { const doubled: number = value * 2; },
};
const numeric = {
  get 0x10() { return true; },
  set 16(value) { const flag: boolean = value; },
};
const setterFirst = {
  set count(value) { const n: number = value; },
  get count() { return 0; },
};
paired.label = "b";
quoted.size = 2;
numeric[16] = false;
setterFirst.count = 1;

const mismatch = {
  get label() { return "a"; },
  set label(value) { const n: number = value; },
};

// A getter without a setter is a read-only property.
const getOnly = { get id() { return 1; } };
getOnly.id = 2;
const readonlyView: { readonly id: number } = getOnly;

// A getter with no annotation of its own returns what its setter accepts.
const fromSetter = {
  get value() { return "text"; },
  set value(next: number) { },
};
const typedFromSetter: number = fromSetter.value;
