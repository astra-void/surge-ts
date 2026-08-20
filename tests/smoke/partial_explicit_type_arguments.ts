interface Meta {
  title?: string;
}
interface Schema {
  kind: string;
}

declare class Registry<T extends Meta = Meta, S extends Schema = Schema> {
  meta: T;
  schema: S;
}

declare function registry<T extends Meta = Meta, S extends Schema = Schema>(): Registry<T, S>;

export const global = registry<Meta>();
export const kind: string = global.schema.kind;
