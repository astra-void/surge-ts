declare function identity<T>(value: T): T;
declare function optional<T>(value: T): T | undefined;
declare function narrowed<T>(value: T): T extends string ? T : never;
declare function boxed<T>(value: T): { value: T };
declare function listed<T>(value: T): T[];
declare function unwrapped<T>(box: { value: T }): T;

// Unannotated, so no contextual return type feeds the inference.
// `T` is the return type at its top level, so the argument's literal survives.
const keptNumber = identity(1);
const keptString = identity("a");
const keptOptional = optional(1);
const keptConditional = narrowed("a");
const numberOk: 1 = keptNumber;
const numberMismatch: 2 = keptNumber;
const stringOk: "a" = keptString;
const optionalOk: 1 | undefined = keptOptional;
const conditionalWidened: "a" = keptConditional;

// Nested in the return type, the literal widens.
const widenedBox = boxed(1);
const boxMismatch: { value: 1 } = widenedBox;
const widenedList = listed("a");
const listMismatch: "a"[] = widenedList;
// A literal inside an object literal argument widened where it was written.
const widenedProperty = unwrapped({ value: 1 });
const propertyMismatch: 1 = widenedProperty;

// A fresh literal return still widens in a mutable declaration.
let mutable = identity(1);
mutable = 2;

declare function excluded<T>(value: T): T extends number ? never : T;
declare function tupleChecked<T>(value: T): [T] extends [string] ? T : never;
// The false branch is the parameter itself; the true branch of a check on `T`
// carries the implied constraint instead, and the literal widens.
const keptFalseBranch = excluded("a");
const falseBranchMismatch: "b" = keptFalseBranch;
const widenedTupleCheck = tupleChecked("a");
const tupleCheckMismatch: "a" = widenedTupleCheck;
