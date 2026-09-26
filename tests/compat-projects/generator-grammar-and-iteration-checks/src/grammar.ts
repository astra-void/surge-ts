export function* overloaded(s: string): Iterable<any>;
export function* overloaded(s: any): Iterable<any> { yield 1; }

export abstract class Shapes {
    abstract *abstractGenerator(): Iterable<any>;
}

export class Builder {
    *constructor() {}
}

export async function* outer() {
    class Holder {
        static {
            for await (const x of []) {}
        }
    }
    return Holder;
}

export function* voidAnnotated(): void { }
