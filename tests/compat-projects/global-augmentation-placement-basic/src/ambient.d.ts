declare module "x" { export const marker: number; }
declare module "dep" { const a: number; export default a; export { a }; }
