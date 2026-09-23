export {};
type Foo = { foo: string };
type Bar = Foo & { bar: string };

function isBarNonNull(x: Foo | Bar | null) {
  return ('bar' in x!);
}
declare const fooOrBar: Foo | Bar;
if (isBarNonNull(fooOrBar)) {
  const t: Bar = fooOrBar;
}

function isDate(x: object) {
  return x instanceof Date;
}
function flakyIsDate(x: object) {
  return x instanceof Date && Math.random() > 0.5;
}
declare let maybeDate: object;
if (isDate(maybeDate)) {
  let t: Date = maybeDate;
} else {
  let t: object = maybeDate;
}
if (flakyIsDate(maybeDate)) {
  let t: Date = maybeDate;
}

function isStringFromUnknown(x: unknown) {
  return typeof x === "string";
}
declare let str: string;
if (isStringFromUnknown(str)) {
  str.charAt(0);
} else {
  let t: never = str;
}

function isShortString(x: unknown) {
  return typeof x === "string" && x.length < 10;
}
if (isShortString(str)) {
  str.charAt(0);
} else {
  str.charAt(0);
}

function assertAndPredicate(x: string | number | Date) {
  if (x instanceof Date) {
    throw new Error();
  }
  return typeof x === 'string';
}
declare let snd: string | number | Date;
if (assertAndPredicate(snd)) {
  let t: string = snd;
}

function reassigned(x: string | number) {
  x = 1;
  return typeof x === "string";
}
declare let sn: string | number;
if (reassigned(sn)) {
  let t: string = sn;
}

function returnsValue(x: string | undefined) {
  return x;
}
declare let maybe: string | undefined;
if (returnsValue(maybe)) {
  let t: string = maybe;
}

function isStringSatisfies(x: string | number) {
  return (typeof x === "string") satisfies boolean;
}
if (isStringSatisfies(sn)) {
  let t: string = sn;
}

const arrowIsString = (x: string | number) => typeof x === "string";
if (arrowIsString(sn)) {
  let t: string = sn;
} else {
  let t: number = sn;
}
