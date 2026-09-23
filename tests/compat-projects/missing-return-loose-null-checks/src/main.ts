export function some(x: boolean): number {
    if (x) {
        return 1;
    }
}
export function none(x: boolean): number {
    if (x) {
        throw new Error();
    }
}
