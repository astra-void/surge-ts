export {};
// TS2314, TS2315
type Box<T> = { value: T }; 
let box: Box = { value: "ok" }; // TS2314

type Name = string; 
let value: Name<string> = "ok"; // TS2315
