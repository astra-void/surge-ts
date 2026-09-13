export interface Table {
  name: string;
  isAlias: boolean;
}

export function fromTheCallsOwnArgument(table: Table, joins: { alias: string }[]): boolean {
  return !((t) => joins.some(({ alias }) => alias === (t.isAlias ? t.name : '')))(table);
}

export function anAnnotatedParameterIsUnchanged(table: Table): string {
  return ((t: Table) => t.name)(table);
}

export function aFunctionExpressionStillWorks(table: Table): string {
  return (function (t) {
    return t.name;
  })(table);
}

export function theParameterReallyHasTheArgumentsType(table: Table): number {
  const asNumber: number = ((t) => t.name)(table);
  return asNumber;
}
