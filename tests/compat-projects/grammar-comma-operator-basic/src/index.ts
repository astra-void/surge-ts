declare const first: number;
declare const second: number;
declare const holder: { property: number };
declare function effectful(): number;

export const identifierLeft = (first, second);
export const unaryLeft = (!first, second);
export const objectLeft = ({ value: 1 }, second);
export const conditionalLeft = (first ? first : first, second);

export const propertyLeft = (holder.property, second);
export const callLeft = (effectful(), second);

export function counted(): number {
  let index = 0;
  let total = 0;
  for (index = 0, total = 0; index < 3; index += 1) {
    total += index;
  }
  return total;
}
