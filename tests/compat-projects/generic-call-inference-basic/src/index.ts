function id<T>(value: T): T {
  return value;
}

function pair<T>(a: T, b: T): T[] {
  return [a, b];
}

function first<T>(items: T[]): T {
  return items[0];
}

let idString: string = id("hello");
let idNumber: number = id(123);
let idMismatch: number = id("hello");

let pairStringArray: string[] = pair("a", "b");
let pairNumberArray: number[] = pair(1, 2);
let pairMismatch: number[] = pair("a", 1);

let firstNumber: number = first([1, 2, 3]);
let firstMismatch: string = first([1, 2, 3]);

let explicitString: string = id<string>("explicit");
let unknownFallback = id(0 as unknown);

id(missingValue);
