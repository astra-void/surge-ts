export = ast;

declare namespace ast {
  interface Node {
    kind: number;
  }
  interface Expression extends Node {
    parenthesized: boolean;
  }
  interface StringLiteral extends Expression {
    text: string;
  }
  interface ImportDeclaration extends Node {
    moduleSpecifier: Expression;
  }
  interface NodeList<T extends Node> extends ReadonlyArray<T> {
    readonly hasTrailingComma: boolean;
  }

  function isImportDeclaration(node: Node): node is ImportDeclaration;
  function isStringLiteral(node: Node): node is StringLiteral;
  function forEachChild<T>(node: Node, visit: (node: Node) => T | undefined): T | undefined;
}
