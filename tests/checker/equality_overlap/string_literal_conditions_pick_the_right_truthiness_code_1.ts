// The empty string is always falsy, so it is TS2873 rather than TS2872.
export function f() {
    if ("abc") {
    }
}
