interface Box { value: number; }
type Wrapped = Box & unknown;
declare const b: Wrapped;
const a = b.value;
const bad = b.missing;
const c: string = b.value;
