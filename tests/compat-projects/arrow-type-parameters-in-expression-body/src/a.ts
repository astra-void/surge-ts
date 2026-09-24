const inArray = [(x: number) => [""], <T, U>(x: T) => x as unknown as U[]];
const inObject = { make: <T, U>(x: T) => x as unknown as U[] };
const asserted = [<T>(x: T) => <T>x];
const unknownName = [<T>(x: T) => x as unknown as V];
