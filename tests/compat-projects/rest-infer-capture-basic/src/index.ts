type Procedure = (...args: any[]) => any;
type Args<T> = T extends (...a: infer A) => any ? A : never;
type Mock<T extends Procedure = Procedure> = { name: string } & { (...args: Args<T>): ReturnType<T> };

declare function spread(...args: Args<(...a: any[]) => void>): void;
declare function fixed(...args: Args<(a: string, b: number) => void>): void;
declare function mixed(...args: Args<(a: string, ...rest: number[]) => void>): void;
declare const mock: Mock;
declare const typed: Mock<(id: number) => string>;

spread('x', 1, true);
spread();
fixed('a', 1);
mixed('a', 1, 2);
mock('anything', 2);
export const result: string = typed(1);

fixed(1, 'a');
typed('not a number');
