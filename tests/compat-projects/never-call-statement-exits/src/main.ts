declare function fail(message?: string): never;

export function viaFunction(x: number): number {
    if (x >= 0) return x;
    fail("negative");
    x;
}

export function viaParameter(x: number, stop: () => never): number {
    if (x >= 0) return x;
    stop();
    x;
}

export class Checker {
    fail(): never {
        throw new Error();
    }
    narrowed(x: string | undefined) {
        if (x === undefined) this.fail();
        return x.length;
    }
    ended(x: number): number {
        if (x >= 0) return x;
        this.fail();
        x;
    }
}

export function falls(x: number): number {
    if (x >= 0) return x;
    x;
}
