namespace Types {
  export type Id = number;
  type Hidden = string;
  export const version = 1;
  export interface Shape {
    kind: string;
  }
}
namespace Empty {}
namespace Values {
  export const value = 1;
}
namespace Reexports {
  interface Local {}
  export { Local };
}

export const typesAsValue = Types.version;
export const emptyAsValue = Empty;
export let valuesAsType: Values;
export let missingMember: Types.Missing;
export let hiddenMember: Types.Hidden;
export let misspelledMember: Types.Shap;
export let valueAsType: Types.version;
export let typeAsNamespace: Types.Shape.kind;
export let typeOfEmpty: typeof Empty;
