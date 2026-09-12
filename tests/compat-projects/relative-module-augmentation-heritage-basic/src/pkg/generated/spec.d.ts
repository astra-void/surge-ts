export declare interface NodeOrTokenData {
  range: [number, number];
}
export declare interface BaseNode extends NodeOrTokenData {
  type: string;
}
export declare interface Identifier extends BaseNode {
  name: string;
}
export declare interface Literal extends BaseNode {
  value: string;
}
export declare type Node = Identifier | Literal;
