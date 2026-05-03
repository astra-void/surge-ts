import defaultValue from "pkg-default";
import getName from "pkg-default-function";
import * as ns from "pkg-ns";
import { User, value } from "barrel-pkg";
import type { User as BarrelUser } from "barrel-type-pkg";
import { User as StarUser, value as starValue } from "barrel-star-pkg";
import { a, b } from "merge-pkg";

let okDefault: string = defaultValue;
let okName: string = getName();
let okNsValue: string = ns.value;
let okNsName: string = ns.getName();
let okUser: User = { name: value };
let okBarrelUser: BarrelUser = { name: value };
let okStarUser: StarUser = { name: starValue };
let okA: string = a;
let okB: number = b;
