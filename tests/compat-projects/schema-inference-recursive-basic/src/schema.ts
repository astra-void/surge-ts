export interface Schema<T> {
  readonly _output: T;
  parse(input: unknown): T;
}

export interface OptionalSchema<T> extends Schema<T | undefined> {
  readonly isOptional: true;
}

export declare function string(): Schema<string>;
export declare function number(): Schema<number>;
export declare function boolean(): Schema<boolean>;
export declare function optional<T>(schema: Schema<T>): OptionalSchema<T>;
export declare function array<T>(item: Schema<T>): Schema<T[]>;
export declare function union<A, B>(a: Schema<A>, b: Schema<B>): Schema<A | B>;
export declare function object<T extends Record<string, Schema<unknown>>>(
  shape: T,
): Schema<{ [K in keyof T]: T[K]["_output"] }>;

export type Infer<S extends Schema<unknown>> = S["_output"];

export declare function lazy<T>(factory: () => Schema<T>): Schema<T>;
