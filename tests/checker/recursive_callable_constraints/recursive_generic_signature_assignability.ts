// @strict: true

interface Recursive<T> { value: T; }

declare let direct: <T extends Recursive<T>>(value: T) => void;
declare let nested: <T extends Recursive<Recursive<T>>>(value: T) => void;
declare let renamedDirect: <U extends Recursive<U>>(value: U) => void;
declare let renamedNested: <U extends Recursive<Recursive<U>>>(value: U) => void;

direct = nested;
nested = direct;
direct = renamedDirect;
renamedDirect = direct;
nested = renamedNested;
renamedNested = nested;
nested = renamedDirect;
renamedNested = direct;

declare let stringValue: <T extends Recursive<string>>(value: T) => T;
declare let numberValue: <U extends Recursive<number>>(value: U) => U;
stringValue = numberValue;
numberValue = stringValue;
