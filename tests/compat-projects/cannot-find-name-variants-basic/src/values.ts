const counter = 1;
export const plain = missingValue;
export const typo = countr;
export const shorthand = { notDeclared };
export const nodeGlobal = require;
export const moduleGlobal = module;
export const jquery = $;
export const bunGlobal = Bun;
describe("suite", () => {});
it("case", () => {});
function notAsync() {
  return await(1);
}
notAsync();

interface Shape {
  size: number;
}
type Alias = string;
export const interfaceAsValue = Shape;
export const aliasAsValue = Alias;
export const primitiveAsValue = string;
export let queried: typeof process;
export let queriedTypo: typeof countr;
