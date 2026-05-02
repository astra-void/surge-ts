import getDefaultName from "./user";
import * as user from "./user";
import { getName, version } from "./reexports";
import type { UserModel } from "./reexports";

let defaultName: string = getDefaultName();
let directName: string = user.getName();
let directVersion: string = user.version;
let namedName: string = getName();
let namedVersion: string = version;
let reexportedUser: UserModel = { name: defaultName };
