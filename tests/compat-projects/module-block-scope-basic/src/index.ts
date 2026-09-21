const outer = 1;
if (Math.random()) {
    const top = 1;
    const name = "x";
    let status = 2;
    const outer = "shadow";
    const shadowed: number = outer;
    const fine: string = name + top + status;
}
{
    const name = 3;
    const wrong: string = name;
}
let y: string | number = Math.random() ? "a" : 1;
if (typeof y === "string") {
    y = 2;
}
const z: number = y;
const n: string = outer;
export {};
