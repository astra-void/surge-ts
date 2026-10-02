export function f() {
const schema = { name: "x" };
type Schema = typeof schema;
const use = (arg: Schema) => arg.name;
return use(schema);
}
