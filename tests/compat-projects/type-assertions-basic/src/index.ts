// Primitive assertions
const a: number = 1 as number;
const b: string = 1 as any;
const c: string = 1 as number; // TS2322

// Alias / interface assertions
type User = { name: string };
const rawUser = { name: "Ada", extra: true };
const user: User = rawUser as User;
const notUser: User = "ada" as any;

interface Person { age: number }
const person: Person = {} as Person;

// Array<T> / ReadonlyArray<T> assertions
const arr1: Array<string> = [] as Array<string>;
const arr2: ReadonlyArray<number> = [] as ReadonlyArray<number>;
const arr3: Array<number> = [] as Array<string>; // TS2322

// Unresolved asserted type should emit TS2304 once
const unknownVal: number = 1 as NonExistentType; // TS2304 for NonExistentType, no TS2322 because it becomes unknown
