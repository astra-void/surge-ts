enum Kind { Identifier = "Identifier", Literal = "Literal" }
interface Identifier { type: Kind.Identifier; name: string }
interface Literal { type: Kind.Literal; value: string }
type Expression = Identifier | Literal;
interface Base { init: Expression | null }
interface MaybeInit extends Base { definite: false }
interface NoInit extends Base { definite: false; init: null }
interface Definite extends Base { definite: true; init: null }
type Declarator = MaybeInit | NoInit | Definite;

export function viaAnd(node: { init: Expression | null }) {
  return node.init?.type === Kind.Identifier && node.init.name;
}

export function viaConditional(node: { init: Expression | null }) {
  return node.init?.type === Kind.Identifier ? node.init.name : "";
}

export function unionBase(node: Declarator) {
  if (node.init?.type === Kind.Identifier) {
    return node.init.name;
  }
  return "";
}

export function unionBaseAnd(node: Declarator) {
  return node.init?.type === Kind.Identifier && node.init.name;
}

export function wrong(node: Declarator) {
  return node.init?.type === Kind.Identifier && node.init.value;
}
