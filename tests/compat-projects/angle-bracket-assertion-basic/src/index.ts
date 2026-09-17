const annotated: string = <number>1;

let constTuple = <const>["a", 1];
constTuple = ["b", 1];

declare function take(x: string): void;
take(<number>2);

const nested = <string>(<unknown>5);
const read: number = nested;

const widened = <string>"x";
const accepted: string = widened;
