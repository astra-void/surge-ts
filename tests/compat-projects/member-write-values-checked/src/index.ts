import * as values from "./values";

declare const frozen: { readonly x: number };
declare const known: { a: number };
declare const loose: any;

frozen.x = missingOne;
known.b = missingTwo;
values.fixed = missingThree;
loose.y = missingFour;

namespace Inner {
    export function write(p: { a: unknown }) {
        p.a = <MissingType>p.a;
    }
}

export { Inner };
