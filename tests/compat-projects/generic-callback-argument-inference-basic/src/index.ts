type Procedure = (...args: Array<any>) => any;

interface Constructable {
  new (...args: Array<any>): any;
}

type MockParameters<T extends Procedure | Constructable> = T extends Constructable
  ? ConstructorParameters<T>
  : T extends Procedure
    ? Parameters<T>
    : never;

type MockReturnType<T extends Procedure | Constructable> = T extends Constructable
  ? InstanceType<T>
  : T extends Procedure
    ? ReturnType<T>
    : never;

interface MockInstance<T extends Procedure | Constructable = Procedure> {
  getMockName(): string;
}

type Mock<T extends Procedure | Constructable = Procedure> = MockInstance<T> &
  (T extends Constructable
    ? { new (...args: ConstructorParameters<T>): InstanceType<T> }
    : { (...args: MockParameters<T>): MockReturnType<T> });

declare function mock<T extends Procedure | Constructable = Procedure>(
  implementation?: T,
): Mock<T>;

async function fetchData<TData>(value: TData): Promise<TData> {
  return value;
}

declare function sleep(ms: number): Promise<void>;

export function run(): void {
  const overGeneric = mock(fetchData);
  void overGeneric('a');

  const chained = mock(() => sleep(10).then(() => 'data'));
  const asChained: () => Promise<string> = chained;
  void asChained;

  const concatenated = mock((value: unknown) => 'data' + String(value));
  const asConcatenated: (value: unknown) => string = concatenated;
  void asConcatenated;
}
