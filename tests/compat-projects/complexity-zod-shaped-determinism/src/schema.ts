export interface Schema<T> {
  parse(input: unknown): T;
  optional(): Schema<T | undefined>;
  array(): Schema<T[]>;
}

export declare function string(): Schema<string>;
export declare function number(): Schema<number>;
export declare function boolean(): Schema<boolean>;
