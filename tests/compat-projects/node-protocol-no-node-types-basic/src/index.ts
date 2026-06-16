import { Buffer } from "node:buffer";
import { readFile } from "fs";
import { thing } from "some-unscoped-package";

console.log(Buffer, readFile, thing);
