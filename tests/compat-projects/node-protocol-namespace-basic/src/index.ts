import * as path from "node:path";

const joined: string = path.join("a", "b");
const separator: string = path.sep;
path.join(123);

console.log(joined, separator);
