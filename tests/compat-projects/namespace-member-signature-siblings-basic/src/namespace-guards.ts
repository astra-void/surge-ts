import * as ast from "../vendor/ast";

export function specifierOf(node: ast.Node) {
  if (ast.isImportDeclaration(node) && ast.isStringLiteral(node.moduleSpecifier)) {
    return node.moduleSpecifier.text;
  }
  return "";
}

export function scan(root: ast.Node) {
  let found = false;
  ast.forEachChild(root, (node) => {
    if (!found && ast.isImportDeclaration(node)) {
      const { moduleSpecifier } = node;
      if (ast.isStringLiteral(moduleSpecifier) && moduleSpecifier.text.length > 0) {
        found = true;
      }
    }
    return undefined;
  });
  return found;
}

export function kindOf(node: ast.Node) {
  return ast.isStringLiteral(node) ? node.text : String(node.kind);
}

export function textOr(node: ast.Node, fallback: string) {
  return !ast.isStringLiteral(node) || node.text.length === 0 ? fallback : node.text;
}
