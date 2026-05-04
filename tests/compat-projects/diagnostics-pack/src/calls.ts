export {};
// TS2349, TS2554, TS2345
function greet(name: string) {}

greet(); // TS2554
greet("a", "b"); // TS2554
greet(1); // TS2345

let non_callable = 1;
non_callable(); // TS2349
