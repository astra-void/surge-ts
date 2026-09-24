type JsonPrimitive = boolean | number | string | null;
type JsonArray = JsonValue[] | readonly JsonValue[];
type JsonObject = {
  readonly [key: string | number]: JsonValue;
  [key: symbol]: never;
};
type JsonValue = JsonPrimitive | JsonObject | JsonArray;

type Post = { id: number; title: string };
declare const post: Post;
declare const posts: Post[];

export const asObject: JsonObject = post;
export const asValue: JsonValue = posts;

type IsJson<T> = T extends JsonValue ? true : false;
export const postIsJson: false = null as unknown as IsJson<Post>;
export const postsAreJson: false = null as unknown as IsJson<Post[]>;
export const dateIsJson: true = null as unknown as IsJson<{ at: Date }>;

interface Bag {
  [key: string]: number;
  [key: symbol]: string;
}
declare const bag: Bag;
export const fromBag: string = bag.anything;

type SymbolsOnly = { [key: symbol]: boolean };
declare const symbolsOnly: SymbolsOnly;
export const missing = symbolsOnly.name;

type Counts = { [key: string | number]: number };
declare const counts: Counts;
export const byIndex: string = counts[0];
export const byName: string = counts.total;
