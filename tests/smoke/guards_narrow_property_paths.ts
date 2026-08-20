interface Args {
  kind: "args";
}
interface Item {
  kind: "item";
}

declare function fromItems(items: Item[]): Args;
declare function fallback(): Args;

export function ternary(params?: { input?: Args | Array<Item> }): Args {
  return Array.isArray(params?.input)
    ? fromItems(params?.input as Item[])
    : (params?.input ?? fallback());
}

export function statement(params: { input: Args | Item[] }): Args {
  if (Array.isArray(params.input)) {
    return fromItems(params.input);
  }
  return params.input;
}

export function instance(params: { value: Date | string }): string {
  if (params.value instanceof Date) {
    return params.value.toISOString();
  }
  return params.value;
}
