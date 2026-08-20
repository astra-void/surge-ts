type Kind = "string" | "number";
interface Schema {
  type?: Kind;
}
type Entry = boolean | Schema;

interface Root {
  items?: Entry | Entry[];
}

export const root: Root = {
  items: [{ type: "string" }, { type: "number" }],
};
