import ClassB = require("./class-target");
export var b1: ClassB = new ClassB();
const b1Bad: ClassB = 1;

import B = require("./dotted-target");
const b2: B = new B(B.b);
const b2Bad: B = 1;

import fooLength = require("./expression-target");
fooLength + 1;
const lengthBad: string = fooLength;
let lengthAsType: fooLength;

import Q = require("./parenthesized-target");
const v: number = Q.v;

import N = require("./namespace-target");
const j: N.J = { j: 1 };
const jBad: N.J = { j: "no" };
const k: string = N.k;

import { X, Shape } from "foobar";
const x: X = X;
const xBad: X = "no";
const shape: Shape = { size: 1 };
const shapeBad: Shape = { size: "no" };
import { Y } from "foobar";

import X2 = require("foobarx");
const x2: X2 = X2;
const x2Bad: X2 = "no";

import FB = require("foobar");
const fb: FB.X = FB.X;
const fbBad: FB.Shape = { size: "no" };

import E from "./default-dotted-target";
const e: E = new E(E.e);
const eBad: E = 1;

import F from "./default-parenthesized-target";
const f = new F();
let fAsType: F;
