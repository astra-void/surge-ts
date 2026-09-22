interface Show {
  show: (value: number) => string;
}
interface Nested {
  nested: Show;
}
interface Tuples {
  pair: [string, number];
}
interface Choice {
  choice: "foo" | "bar";
}

export function renamed({ show: render = (value) => value }: Show) {
  return render;
}
export function quoted({ "show": render = (value) => value }: Show) {
  return render;
}
export function nestedValue({ nested: inner = { show: (value) => value } }: Nested) {
  return inner;
}
export function nestedPattern({ nested: { show = (value) => value } }: Nested) {
  return show;
}
export function tuple({ pair = [101, 1234] }: Tuples) {
  return pair;
}
export function literal({ choice = "baz" }: Choice) {
  return choice;
}
export const arrow = ({ choice = "qux" }: Choice) => choice;

export function accepted({
  show = (value: number) => String(value),
  pair = ["a", 1],
  choice = "foo",
}: Show & Tuples & Choice) {
  return [show, pair, choice];
}
export function generic<TData>({
  reducer = (items: TData[], chunk: TData) => items.concat(chunk),
  limit = "many",
}: {
  reducer?: (items: TData[], chunk: TData) => TData[];
  limit?: number;
}) {
  return [reducer, limit];
}
export function optionalParent({
  method = "z",
  nested: { inner = "c" },
}: {
  method?: "x" | "y";
  nested?: { inner: "a" | "b" };
}) {
  return [method, inner];
}

export const widenedFromArgument = (({ count = 14 }) => count)({ count: 15 });
