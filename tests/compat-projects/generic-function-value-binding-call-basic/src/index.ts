function identity<T>(value: T): [T] {
  return [value];
}
const alias = identity;
const first: string = alias(1)[0];

declare const direct: <T>(value: T) => { value: T };
const copied = direct;
copied(1).value.toUpperCase();

const twice = alias;
twice('x')[0].missing;
