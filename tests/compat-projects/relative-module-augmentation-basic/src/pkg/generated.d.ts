export declare interface Identifier {
  kind: "identifier";
  name: string;
}
export declare interface Block {
  kind: "block";
  body: string;
}
export type Node = Identifier | Block;
