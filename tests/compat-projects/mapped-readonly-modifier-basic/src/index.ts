type User = { id: number; tags: string[]; label?: string };
type Frozen<T> = { readonly [K in keyof T]: T[K] };
type Thawed<T> = { -readonly [K in keyof T]: T[K] };

declare const libReadonly: Readonly<User>;
declare const customReadonly: Frozen<User>;
declare const keyed: { readonly [K in "a" | "b"]: number };
declare const thawed: Thawed<{ readonly id: number }>;
declare const kept: { [K in keyof Frozen<User>]: Frozen<User>[K] };

// The `readonly` modifier makes every mapped property read-only.
libReadonly.id = 1;
customReadonly.id = 2;
keyed.a = 3;
function parameter(user: Readonly<User>): void {
  user.label = "x";
  user.tags.push("still mutable inside");
}

// `-readonly` removes it, and no modifier keeps the source's.
thawed.id = 4;
kept.id = 5;

// Reading and assigning between the shapes stays legal.
const copy: User = libReadonly;
const frozen: Readonly<User> = copy;
