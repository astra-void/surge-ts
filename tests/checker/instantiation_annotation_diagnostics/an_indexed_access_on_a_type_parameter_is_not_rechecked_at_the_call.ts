// A generic call re-resolves the declaration's written annotations under the
// call's substitution. That is not a fresh declaration check — the declaration
// was checked where it was written, against the type parameter's *constraint* —
// so nothing raised there belongs to the call. `(o: K[1])` under `K = [""]`
// reported a false TS2493 on a parameter list the call never looked at.
declare function at<K extends [string, number?]>(k: K, o: K[1]): void;
at([""], undefined);
