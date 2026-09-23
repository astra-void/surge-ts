declare function take(value: { a: true }): void;
declare function takeList(values: string[]): void;

take({ a: 1 } satisfies unknown);
takeList([1] satisfies unknown[]);
const plain = { a: 1 };
take(plain satisfies unknown);
take({ a: true } satisfies { a: boolean });
