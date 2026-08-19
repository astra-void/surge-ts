interface BaseDef {
  kind: string;
}

interface AnyDef extends BaseDef {
  kind: "any";
}

interface BaseSchema {
  transform: (def: BaseDef) => void;
}

interface AnySchema {
  transform: (def: AnyDef) => void;
}

declare const anySchema: AnySchema;
let schema: BaseSchema = anySchema;
export { schema };
