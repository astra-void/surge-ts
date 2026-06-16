import { Buffer } from "node:buffer";

const buf = new Buffer();
const size: number = buf.length;
const written: number = buf.write("payload");
buf.write(123);

console.log(size, written);
