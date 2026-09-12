declare function eachRule(create: (context: number) => unknown): void;

export function createOrderRule<TFunc extends string, TProp extends string>(
  targetFunctions: ReadonlyArray<TFunc>,
  orderRules: ReadonlyArray<TProp>,
) {
  const targetSet = new Set<string>(targetFunctions);

  // The predicate's target names the *enclosing* function's type parameter.
  // Narrowing re-resolves this annotation at the guard site below, which must
  // still see `TFunc`.
  function isTargetFunction(node: string): node is TFunc {
    return targetSet.has(node);
  }

  eachRule((context) => {
    return {
      CallExpression(node: { name: string }) {
        if (!isTargetFunction(node.name)) {
          return;
        }
        return orderRules.indexOf(node.name as unknown as TProp) + context;
      },
    };
  });
}

// Not a suppression: the enclosing parameter is still a real name elsewhere.
export const bad: string = createOrderRule(['a'], ['b']) as unknown as number;
