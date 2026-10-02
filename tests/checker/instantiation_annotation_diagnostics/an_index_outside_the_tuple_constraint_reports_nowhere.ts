// tsc does not check a type parameter's indexed access against the arity of its
// tuple constraint either — `K[1]` under `K extends [string]` resolves rather
// than reporting — so the suppression costs nothing here.
declare function at<K extends [string]>(k: K, o: K[1]): void;
at([""], undefined);
