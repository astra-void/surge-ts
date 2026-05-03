import { value, getName, PackageUser } from "pkg";
import { subValue } from "pkg/subpath";
import { missing } from "missing-pkg";

let id: ID = "ok";
let badId: ID = 123;

let user: User = { name: "Ada" };
let badUser: User = { name: 123 };

let packageUser: PackageUser = { name: value };
let name: string = getName(packageUser);
let badName: number = getName(packageUser);

let n: number = subValue;
console.log(name);
let timer: number = setTimeout(process, 1);

let fallback = missing;
