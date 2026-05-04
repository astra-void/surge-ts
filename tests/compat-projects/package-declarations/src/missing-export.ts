import { Missing } from "pkg";
import { MissingSub } from "subpath-pkg/feature";
import { version, makeUser } from "pkg";
import * as pkg from "pkg";

const x: number = version;
makeUser(123);
makeUser();
pkg.missing;