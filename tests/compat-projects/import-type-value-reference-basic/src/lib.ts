export interface Options {
  upper: boolean;
}

export function greet(name: string, options?: Options): string {
  return options?.upper ? name.toUpperCase() : name;
}

export const registry = { greet };
