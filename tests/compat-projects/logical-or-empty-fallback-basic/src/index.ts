function direct(o?: { color?: string; width?: number }, r?: { color: string }) {
    const x1 = (o || {}).color;
    const x2 = (r || {}).color;
    const x3 = (r ?? {}).color;
    const n: number = x2;
    const m: number = x3;
    (r || {}).nope;
    return x1;
}
function destructured(o?: { color?: string; width?: number }) {
    const { color, width } = o || {};
    const s: string = color;
    return width;
}
function element(o?: { color: string }) {
    return (o || {})["color"];
}
