declare const pair: [number, string];
declare function makePair(): [number, string];
declare const holder: { single: [number] };

const pastEnd = pair[2];
const negative = pair[-1];
const [first, second, third] = pair;
const fromCall = makePair()[2];
const [c1, c2, c3] = makePair();
const nested = holder.single[1];
const [n1, n2] = holder.single;

const [d1, d2, defaulted = "fallback"] = pair;
const defaultedText: string = defaulted;
const inRange: string = pair[1];
