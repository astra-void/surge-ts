type Post = { id: number; title: string };
type ElementOf<T> = T extends readonly (infer U)[] ? U : "none";

const fromArray: number = null as unknown as ElementOf<Post[]>;
const fromStrings: number = null as unknown as ElementOf<string[]>;
const fromReadonly: number = null as unknown as ElementOf<readonly string[]>;
const fromNonArray: number = null as unknown as ElementOf<Post>;
export {};
