import * as values from "./values";

const local = 1;
function helper() {}
namespace Types {
  export type Id = number;
}
type Point = { x: number };
interface Named {
  name: string;
}

export let valueAsType: local;
export let functionAsType: helper;
export let namespaceAsType: Types;
export let importAsType: values;
export let missingType: MissingType;
export let typoType: Poimt;
export let missingNamespace: Missing.Inner;
export let nodeNamespace: NodeJS.Timeout;
export let typeAsNamespace: Point.y;
export let propertyOfType: Named.name;
export let spelledNamespace: Intll.Collator;
export let buffer: Buffer;
helper();
