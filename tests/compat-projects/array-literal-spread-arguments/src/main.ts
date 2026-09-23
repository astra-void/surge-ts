declare function six(a: number, b: number, c: number, d: number, e: number, f: number): void;
six(1, 2, 3, 4, ...[5, 6]);
six(...[1], 2, 3, 4, 5, 6);
six(1, 2, ...[3, 4], 5, 6);
six(...([1, 2]), ...[3, 4], ...[5, 6]);
six(...[1, 2, 3]);
declare const numbers: number[];
six(...numbers);
export {};
