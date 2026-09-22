export function operands(
  flag: boolean,
  text: string,
  count: number,
  nullable: string | null,
  optional: { size: number } | undefined,
) {
  const andKeepsFalse: false | string = flag && text;
  const andKeepsEmpty: "" | number = text && count;
  const andKeepsZero: 0 | string = count && text;
  const andKeepsNull: number | "" | null = nullable && nullable.length;
  const andKeepsUndefined: number | undefined = optional && optional.size;
  const orKeepsTrue: true | number = flag || count;
  const orDropsNullish: string = nullable || text;
  const orDropsUndefined: { size: number } | string = optional || text;

  const andNotTrue: true | string = flag && text;
  const andNotNullFree: number | string = nullable && nullable.length;
  const orNotFalse: false | number = flag || count;
  return [
    andKeepsFalse,
    andKeepsEmpty,
    andKeepsZero,
    andKeepsNull,
    andKeepsUndefined,
    orKeepsTrue,
    orDropsNullish,
    orDropsUndefined,
    andNotTrue,
    andNotNullFree,
    orNotFalse,
  ];
}
