# global-augmentation-merge-base-scope

Three collections have to run in this order, and this fixture fails if either
adjacent pair is swapped:

1. ambient global **types**,
2. `declare global` augmentation **types**,
3. ambient global **values**.

A merged interface takes its declaring file and resolution scope from whichever
fragment merged *first*, and only the ambient fragment's scope can resolve the
member annotations. `augment.d.ts` gives the same name a module-local meaning
(`type DomNodeDetail = "module-local-meaning"`), so if the augmentation wins the
merge base, `DomNode.detail` resolves to that string literal and every member
read through it is a false `TS2339`. That is what running the augmentation
collection first cost on tRPC: `lib.dom.d.ts::Node` degraded 309,348 times,
degraded members are never cached, and peak memory doubled.

The value pass has to come *after* the augmentation types, which is the half
[`ambient-script-var-global-augmented-type`](../ambient-script-var-global-augmented-type)
pins directly and this fixture pins again through `domRoot.augmented`: a script
`globals.d.ts` declares `var domRoot: DomNode` while the member comes from a
module's `declare global`, so lowering the value first freezes it against the
partial interface.

The single intentional error keeps the members honest: `detail.kind` really is
`"element"`, so binding it to a `number` reports.
