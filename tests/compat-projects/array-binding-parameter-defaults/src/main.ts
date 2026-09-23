export function noInitializer([x = 0, y]) {}
export function shortInitializer([x, y] = [1]) {}
export function emptyInitializer([x, y = "b"] = []) {}
export function fullInitializer([x, y] = [1, "a"]) {}
export function nested([a, [b = 1, c]]) {}
export function objectDefaults({ p = 1, q }) {}
export const arrow = ([m = 1, n]) => {};
