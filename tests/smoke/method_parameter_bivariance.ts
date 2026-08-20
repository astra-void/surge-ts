interface BaseDef {
  kind: string;
}

interface AnyDef extends BaseDef {
  kind: "any";
}

interface BaseSchema {
  clone(def: BaseDef): BaseSchema;
  transform: (def: BaseDef) => BaseSchema;
}

interface AnySchema {
  clone(def: AnyDef): BaseSchema;
  transform: (def: BaseDef) => BaseSchema;
}

declare const anySchema: AnySchema;
let schema: BaseSchema = anySchema;
export { schema };
