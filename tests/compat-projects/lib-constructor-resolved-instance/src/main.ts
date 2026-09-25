const typed = new Int8Array(4);
const shared: SharedArrayBuffer = typed.buffer;
const sharedView: Int8Array<SharedArrayBuffer> = typed;

const buffer = new SharedArrayBuffer(8);
const bytes = new Uint8Array(buffer);
const plainView: Uint8Array<ArrayBuffer> = bytes;

const cells = new Array<string>(2);
const count: number = cells;
