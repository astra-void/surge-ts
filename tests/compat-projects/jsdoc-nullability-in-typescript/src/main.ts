export function postfix(a: string!) {
    return a.length;
}

export function prefix(a: !number) {
    return a.toFixed();
}

export function nullablePostfix(a: string?) {
    return a;
}

export function nullablePrefix(a: ?number) {
    return a;
}

export function returns(): string! {
    return "value";
}

export const narrowed: number = nullablePrefix(1);
