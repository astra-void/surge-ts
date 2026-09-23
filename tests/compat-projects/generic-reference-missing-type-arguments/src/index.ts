import a = require("./generic");
var v: a;
var ok: a<number> = new a<number>();
import { G, I as II, Pair } from "./lib";
var x: G;
var y: II;
var p: Pair<string>;
var pOk: Pair<string, number>;
class Local<T> { l!: T }
var z: Local;
var zz: Local<string, number>;
var zOk: Local<string>;
