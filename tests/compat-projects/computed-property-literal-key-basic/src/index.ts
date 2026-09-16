const key = "alpha";
declare const declaredKey: "beta";
declare const anyString: string;
const KEYS = { gamma: "gamma" } as const;
enum Names { Delta = "delta" }

// A computed key whose type is a literal names that property.
const fromConst: { alpha: number } = { [key]: 1 };
const fromDeclared: { beta: number } = { [declaredKey]: 1 };
const fromMember: { gamma: number } = { [KEYS.gamma]: 1 };
const fromEnum: { delta: number } = { [Names.Delta]: 1 };
const fromTemplate: { epsilon: number } = { [`epsilon`]: 1 };

// ...so it is not an excess property, and a missing one is still missing.
const excess: { alpha: number } = { [key]: 1, [declaredKey]: 2 };
const missing: { alpha: number; beta: number } = { [key]: 1 };

// A `string` key adds no named member.
const broad: { alpha: number } = { alpha: 1, [anyString]: 2 };

const inferred = { [key]: 1 };
const readBack: string = inferred.alpha;
