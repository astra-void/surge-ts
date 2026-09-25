/**
 * @param {string} a
 * @param {number} [b]
 * @param {boolean=} c
 * @returns {number}
 */
function f(a, b, c) {
    return a;
}
f(1);
f("x", 1, true, 4);

/** @type {number} */
var n = "s";

const v = /** @type {number} */ ("x");

/**
 * @template T
 * @param {T} x
 * @returns {T}
 */
function id(x) { return x; }
/** @type {string} */
const r = id(1);

/**
 * @typedef {Object} Point
 * @property {number} x
 * @property {number} [y]
 */
/** @type {Point} */
const p = { x: "a" };

/**
 * @callback Cb
 * @param {string} name
 * @returns {void}
 */
/** @type {Cb} */
const cb = (name) => { name.toFixed(); };

/**
 * @param {Object} opts
 * @param {string} opts.name
 */
function g(opts) { opts.name.toFixed(); }

/** @param {...number} rest */
function h(...rest) { rest.push("x"); }

/** @type {Object.<string, number>} */
const rec = { a: "b" };

/** @type {String} */
const str = 1;

/** @type {*} */
const anything = 1;
anything.foo;

/** @template T */
class Box {
    /** @param {T} value */
    constructor(value) {
        /** @type {T} */
        this.value = value;
    }
    /** @returns {T} */
    get() { return this.value; }
}
/** @type {Box<number>} */
const box = new Box("x");

class K {
    /** @type {number} */
    count = "zero";
}

function ret() {
    /** @type {(a: number) => number} */
    return (a) => a;
}

function untyped(a, b) {}
untyped();

function plain() {
    this.x = 1;
}

[1, 2].map((x) => x.toFixed());

try { } catch (/** @type {unknown} */ err) { err.foo; }
