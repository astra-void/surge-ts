type Plain = { value: string };
declare const g: Plain;
const x = ~g.missing;
export { x };
