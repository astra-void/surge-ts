// What a write is checked against, following tsc's write path: the setter's
// type for an accessor pair, `isReadonlySymbol` before assignability, a tuple's
// element at a literal index with its bounds errors, and the union of a union
// receiver's members.

class Settings {
  #level = 0;
  get level(): number {
    return this.#level;
  }
  set level(next: number | string) {
    this.#level = Number(next);
  }
  get label(): string {
    return "settings";
  }
  readonly id: number = 1;
  name: string = "";
}

const settings = new Settings();
settings.level = "3";
settings["level"] = 4;
settings.level = true;
settings.id = 2;
settings["id"] = 3;
settings.label = "other";
settings.name = 5;

const tuple: [number, string] = [1, "a"];
tuple[0] = 2;
tuple[1] = "b";
tuple[2] = 3;
tuple[-1] = 4;

declare const frozen: readonly number[];
frozen[0] = 1;

declare const frozenTuple: readonly [number, string];
frozenTuple[0] = 2;

declare const either: number[] | string[];
either[0] = 1;
either[0] = true;

const counters: Record<string, number> = {};
counters["hits"] = 1;
counters["hits"] += 1;
counters["hits"] += "one";

export { settings, tuple, counters };

// `isReadonlySymbol` reaches every form the declaration can take, not just a
// class field: an interface member, a type alias, an inline annotation, a
// parameter property, and an `as const` literal.
interface Frozen {
  readonly id: number;
  get label(): string;
  get level(): number;
  set level(next: number | string);
}
declare const frozenInterface: Frozen;
frozenInterface.id = 2;
frozenInterface.label = "other";
frozenInterface.level = "3";
frozenInterface.level = true;

type FrozenAlias = { readonly key: string };
declare const frozenAlias: FrozenAlias;
frozenAlias.key = "other";

declare const frozenInline: { readonly inline: number };
frozenInline.inline = 2;

class WithParameterProperty {
  constructor(
    public readonly slug: string,
    public title: string,
  ) {}
}
const withParameterProperty = new WithParameterProperty("a", "b");
withParameterProperty.slug = "c";
withParameterProperty.title = "d";

const constant = { mode: "fast", sizes: [1, 2] } as const;
constant.mode = "slow";

// A readonly array is not assignable to a mutable one, and that has its own
// code; inferring through `ReadonlyArray<T>` keeps the literal elements.
declare function mutate(values: number[]): void;
declare const readonlyValues: readonly number[];
mutate(readonlyValues);
declare function firstOf<T>(values: ReadonlyArray<T>): T;
const modes = ["fast", "slow"] as const;
export const mode: "fast" | "slow" = firstOf(modes);

// A key that is itself a union writes against the intersection of what each
// names, and a generic receiver can only be indexed for reading.
declare const twoTypes: { a: number; b: string };
declare const eitherKey: "a" | "b";
twoTypes[eitherKey] = 1;

export function writeThroughGeneric<T extends Record<string, number>>(
  target: T,
  key: string,
) {
  target[key] = 1;
}
