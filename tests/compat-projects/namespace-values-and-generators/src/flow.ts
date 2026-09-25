let pending;
export const early = pending < 1;

declare const holder: undefined | { name: string; nick?: string };
delete holder?.name;
delete holder?.nick;

declare class Ambient {
    method(x): void;
    private hidden(y): void;
}

class Open { value = ""; }
class Guarded { protected value = ""; }
declare const either: Open | Guarded;
either.value;

export function* numbers(): Generator<number> {
    yield "one";
    yield* ["two"];
}

export async function* later(): AsyncGenerator<number> {
    yield "one";
}
