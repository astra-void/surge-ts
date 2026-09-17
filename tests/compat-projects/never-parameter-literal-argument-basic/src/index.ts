declare function unreachable(value: never): void;

unreachable(1);
unreachable("text");
unreachable({ a: 1 });
unreachable([1]);
unreachable(`template`);

type Numeric = (value: number) => string;
type Textual = (value: string) => string;
declare const either: Numeric | Textual;
either("x");
