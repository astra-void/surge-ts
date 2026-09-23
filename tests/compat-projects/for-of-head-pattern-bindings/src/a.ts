declare const pairs: Iterable<[key: string, value: number]>;
for (const { 0: key, 1: value } of pairs) {
    key.length;
    value.toFixed();
}
class Table {
    constructor(entries: [key: string, value: number][]) {
        for (const { 0: key, 1: value } of entries) {
            const wrong: number = key;
            value.toFixed();
        }
    }
}
declare const tuple: [string, number];
const { 0: first, 1: second, "0": quoted } = tuple;
first.length;
second.toFixed();
quoted.length;
function read({ 1: count }: [string, number]): string {
    return count;
}
