export {};
// TS2362, TS2363, TS2365, TS2367
let a = {} + 1; // TS2362 (Wait, maybe just `{} * 1` for arithmetic?)
let b = 1 * {}; // TS2363? Actually, left-hand TS2362 and right-hand TS2363 are for arithmetic operators

let c = {} * 1;
let d = 1 * {};

let e = 1 === "string"; // TS2367
