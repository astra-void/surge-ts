export function withDefaults({ first, second = 1, third = "x" }) {
    return [first, second, third];
}
export function nested({ outer: { inner, fallback = true } }) {
    return [inner, fallback];
}
