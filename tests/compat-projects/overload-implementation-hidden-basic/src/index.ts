import { arity, pair, parse } from "./overloads";

export const text: string = parse("a");
export const count: number = parse(1);
export const wrongReturn: boolean = parse("s");
parse(true);
parse();
parse(1, 2);

pair("a", 1);
pair(1, "a");
pair(1, 2);
pair("a", "b");
pair(true, 1);

arity("a");
arity(1, 2);
arity(true);
arity(1);

function local(value: string): string;
function local(value: number): number;
function local(value: unknown): unknown {
  return typeof value === "string" ? local(1) : value;
}

export const viaLocal = [local("a"), local(1)];
local({});
