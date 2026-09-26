const [...items,] = [1, 2];
const { ...others, } = { a: 1 };
let left: number[] = [];
let right: { a?: number } = {};
[...left,] = [1];
({ ...right, } = { a: 1 });

export function collect(...values: number[],) {
    return values;
}

declare function ambient(...values: number[],): void;

const [first, second,] = [1, 2];
const { a, } = { a: 1 };

export { items, others, left, right, first, second, a, ambient };
