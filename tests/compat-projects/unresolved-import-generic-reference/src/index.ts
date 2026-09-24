import { MyPromise } from "missing";
import Missing = require("missing-too");

declare var mp: MyPromise<number>;
async function f3(): MyPromise<void> { }
let nested: MyPromise<MyPromise<string>>;
let unresolvedArgument: MyPromise<NotDeclared>;
let fromEquals: Missing<number>;

interface Plain { p: number }
let plain: Plain<number>;
