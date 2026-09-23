function f() {
  export { x };
  export * from "./dep";
  export const y = 1;
}
const x = 1;
{
  export { x as z };
}
export { x };
export * from "./dep";
