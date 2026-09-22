export {};

class Private {
  #shared = 1;
  static #shared = 2;
  #plain = 1;
  get #pair() {
    return 1;
  }
  set #pair(value: number) {}
  static #method() {}
  #method() {}
}

let direct: typeof direct;
let first: typeof second;
let second: typeof first;
let member: { x: typeof member };
let array: (typeof array)[];
let argument: Array<typeof argument>;
let signature: () => typeof signature;

function selfConstraint<T extends T>() {}
function mutual<T extends U, U extends T>() {}
function intoCycle<T extends U, U extends V, V extends U>() {}
function throughArray<T extends U[], U extends T>() {}

type Loose = unique symbol;
let mutableUnique: unique symbol;
declare const constUnique: unique symbol;
class Holder {
  static readonly ok: unique symbol;
  bad!: unique symbol;
}
interface Signature {
  readonly ok: unique symbol;
  bad: unique symbol;
}

function rest(...args) {}
declare function decorate(...a: any[]): any;
function decorated(@decorate x: number) {}
namespace Loader {
  import fs = require("fs");
}

function forward(a = b, b = 1) {}
function itself(x: number, y: number = y) {}
function deferred(a = () => b, b = 1) {}
const arrow = (x = y + 1, y = 2) => x;

declare function renamed({ a: string }): void;
type RenamedType = ({ a: number }) => void;
interface RenamedMethod {
  m({ a: b }): void;
}
