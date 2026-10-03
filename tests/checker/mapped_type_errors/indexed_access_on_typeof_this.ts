// drizzle's `this.query = {} as typeof this['query']`: surge does not model
// `typeof this`, and indexing the unmodelled receiver reports nothing.
export class Database<T> {
  query: T extends string ? number : { value: T };
  count: number;
  constructor() {
    this.query = {} as typeof this["query"];
    this.count = 1 as typeof this["count"];
  }
}
