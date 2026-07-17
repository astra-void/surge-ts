interface Model {
  id: string;
  tags: string[];
  meta: { created: number; author: { name: string } };
}

type Nullable<T> = { [K in keyof T]: T[K] | null };
type ElementOf<T> = T extends (infer E)[] ? E : T;
type AuthorName = Model["meta"]["author"]["name"];

const nullable: Nullable<Model> = { id: null, tags: null, meta: null };
const tag: ElementOf<Model["tags"]> = "x";
const scalar: ElementOf<number> = 1;
const mixedText: ElementOf<string[] | number> = "a";
const mixedCount: ElementOf<string[] | number> = 1;
const deep: AuthorName = "name";

interface Tree<T> {
  value: T;
  children: Tree<T>[];
}

declare function flatten<T>(root: Tree<T>): T[];

const numbers: number[] = flatten({
  value: 1,
  children: [{ value: 2, children: [] }],
});

type PickByKey<T, K extends keyof T> = { [P in K]: T[P] };
const summary: PickByKey<Model, "id" | "tags"> = { id: "1", tags: [] };

const bad: number = deep;

export { nullable, tag, scalar, mixedText, mixedCount, numbers, summary, bad };
