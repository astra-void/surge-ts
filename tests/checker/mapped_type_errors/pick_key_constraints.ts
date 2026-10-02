// @strict: true

interface Shape { name: string; width: number; }
type Missing = Pick<Shape, "missing">;
type Mixed = Pick<Shape, "name" | "missing">;
type Nullable = Pick<Shape, undefined>;
type Valid = Pick<Shape, "name">;
type Empty = Pick<Shape, never>;
type Open = Pick<Record<string, number>, "anything">;
function unconstrained<T>() { let value: Pick<Shape, T>; }
function primitive<T extends string | number>() { let value: Pick<Shape, T>; }
function constrained<T extends keyof Shape>() { let value: Pick<Shape, T>; }
