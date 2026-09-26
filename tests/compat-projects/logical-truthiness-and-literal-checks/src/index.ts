export function truthiness(required: () => boolean, other: () => boolean, flag: boolean, optional?: () => void) {
    required && console.log("always");
    required && required();
    optional && optional();
    const value = required && 1;
    if ((required || other) && flag) {
    }
    return other && flag;
}

export class Holder {
    run() {
        return true;
    }
    check() {
        this.run && console.log("always");
        this.run && this.run();
    }
}

let pattern = /(?<year>\d{4})/u;
let match = pattern.exec("2015");
export const year = match[0];
export const replaced: number = "abc".replace(/b/g, "x");

class Guarded {
    private secret = 1;
    protected shared = 2;
    peek({ secret }: Guarded) {
        return secret;
    }
}
const guarded = new Guarded();
const { secret } = guarded;
const { shared } = guarded;
export function reveal({ secret: hidden }: Guarded) {
    return hidden;
}

function identity<T>(x: T) {
    return x;
}
let hello = identity<"hello">("hello");
hello = "other";

declare const either: { a: number } | { b: string };
let spread: { a: number } | { b: string } = { ...either };
export const merged: { a: boolean } | { b: string; a: boolean } = { ...either, a: false };
export { secret, shared, hello, spread };
