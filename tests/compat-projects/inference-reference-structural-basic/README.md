# inference-reference-structural-basic

tsc infers from type arguments only between references to the same generic
declaration (`inferFromObjectTypes`, `source.target == target.target`); a
reference to another declaration is inferred from as the structure it expands
to. surge zipped any reference's arguments by position, so `Flipped<string,
number>` against `Box<T>` bound `T` to `string`.
