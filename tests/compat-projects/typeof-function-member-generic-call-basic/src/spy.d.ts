type Procedure = (...args: any[]) => any;
type Constructable = abstract new (...args: any) => any;
type MockParameters<T extends Procedure | Constructable> = T extends Constructable ? ConstructorParameters<T> : T extends Procedure ? Parameters<T> : never;
type MockReturnType<T extends Procedure | Constructable> = T extends Constructable ? InstanceType<T> : T extends Procedure ? ReturnType<T> : never;
interface MockInstance<T extends Procedure | Constructable = Procedure> { mockClear(): this; mockReturnValue(value: MockReturnType<T>): this; }
type Mock<T extends Procedure | Constructable = Procedure> = MockInstance<T> & (T extends Constructable ? (T extends Procedure ? {
	new (...args: ConstructorParameters<T>): InstanceType<T>;
	(...args: Parameters<T>): ReturnType<T>;
} : {
	new (...args: ConstructorParameters<T>): InstanceType<T>;
}) : {
	new (...args: MockParameters<T>): MockReturnType<T>;
	(...args: MockParameters<T>): MockReturnType<T>;
}) & { [P in keyof T] : T[P] };
declare function fn<T extends Procedure | Constructable = Procedure>(originalImplementation?: T): Mock<T>;
interface VitestUtils { fn: typeof fn; }
export declare const vi: VitestUtils;
export { fn };
