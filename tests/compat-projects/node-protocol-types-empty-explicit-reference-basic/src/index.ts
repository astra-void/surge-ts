/// <reference types="node" />
import * as path from "node:path";

const joined: string = path.join("a", "b");
path.join(123);

console.log(joined);
