type IntersectionError<K extends string> = `collision:${K}`;

type ProtectedIntersection<TType, TWith> = keyof TType & keyof TWith extends never
  ? TType & TWith
  : IntersectionError<string & keyof TType & keyof TWith>;

type A = {
  withTRPC: () => void;
};

type B = {
  router99: unknown;
};

type C = ProtectedIntersection<A, B>;

declare const c: C;

export function run(): unknown {
  c.withTRPC();
  return c.router99;
}

type Collides = ProtectedIntersection<A, { withTRPC: () => void }>;

declare const collided: Collides;

export const collision: string = collided;
