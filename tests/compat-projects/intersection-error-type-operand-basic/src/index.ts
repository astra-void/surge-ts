import { Missing } from "missing-module";

type Props = Missing & { label: string };
export const props: Props = { label: "x", extra: 1 };

type Generic<T> = Missing & { value: T };
export const generic: Generic<number> = { value: "x", other: true };

declare function take(options: Missing & { id: number }): void;
take({ id: "1", unknown: [] });

type Known = { a: string } & { b: number };
export const known: Known = { a: "a", b: 1, c: true };
