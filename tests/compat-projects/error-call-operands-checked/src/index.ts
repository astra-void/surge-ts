missingCall(missingArgument, (parameter) => parameter);
missingGeneric<MissingType>(1);

declare const untyped: any;
untyped<AlsoMissing>(2);

declare const receiver: { known(value: number): void };
receiver.unknownMember(missingInMember, function (value) { return value; });

missingObject[missingIndex];
missingTag`${missingInTemplate}`;

for (missingHead in receiver) {}

const values = [1, 2, 3].values();
const doubled: IteratorObject<number, undefined, unknown> = values.map((value) => value * 2);
const collected: number[] = values.filter((value) => value > 1).toArray();
values.notAnIteratorMember();
