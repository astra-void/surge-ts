export interface StandardSchemaV1<Input = unknown, Output = Input> {
  readonly '~standard': StandardSchemaV1.Props<Input, Output>;
}
export declare namespace StandardSchemaV1 {
  export interface Props<Input = unknown, Output = Input> {
    readonly version: 1;
    readonly vendor: string;
    readonly validate: (value: unknown) => Result<Output> | Promise<Result<Output>>;
    readonly types?: Types<Input, Output> | undefined;
  }
  export type Result<Output> = SuccessResult<Output> | FailureResult;
  export interface SuccessResult<Output> {
    readonly value: Output;
    readonly issues?: undefined;
  }
  export interface FailureResult {
    readonly issues: ReadonlyArray<{ message: string }>;
  }
  export interface Types<Input = unknown, Output = Input> {
    readonly input: Input;
    readonly output: Output;
  }
}
type SIn<T> = T extends StandardSchemaV1<infer I, infer O> ? [I, O] : 'no';
declare const direct: StandardSchemaV1<{ a: string }, { b: number }>;
const y1: 0 = null! as SIn<typeof direct>;
interface Schema<T> {
  '~standard': StandardSchemaV1.Props<T, T>;
  parse(x: unknown): T;
}
declare const schema: Schema<{ a: string }>;
const y2: 0 = null! as SIn<typeof schema>;
type Simple<T> = T extends { v: infer V } ? [V] : 'no';
const y4: 0 = null! as Simple<{ v: string }>;
type Nested<T> = T extends { p: { q: infer Q } } ? [Q] : 'no';
const y5: 0 = null! as Nested<{ p: { q: number } }>;
