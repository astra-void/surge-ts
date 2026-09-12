type Procedure = (...args: any[]) => any;

interface MockInstance<T extends Procedure = Procedure> {
  mockClear(): this;
}

type Mock<T extends Procedure = Procedure> = MockInstance<T> & {
  (...args: Parameters<T>): ReturnType<T>;
};

declare function listen(handler: (value: string) => void): void;

// A rest annotation written as a library alias resolves lazily; the
// comparison must still see the array it stands for.
declare const open: (...args: Parameters<Procedure>) => void;
listen(open);

// The vitest shape: an intersection whose call signature spells its rest
// parameter through the alias.
declare const spy: Mock<Procedure>;
listen(spy);
export const cleared: unknown = spy.mockClear();
export const result: unknown = spy('any', 'args');

// A plainly declared mismatch still reports.
listen((value: number) => {});
