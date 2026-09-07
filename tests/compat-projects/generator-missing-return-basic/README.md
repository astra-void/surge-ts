# generator-missing-return-basic

A generator's declared return type describes what it *yields*, so tsc requires
no `return` statement in it. surge ran the ordinary missing-return analysis and
reported `TS2355` on every annotated `function*` — the shape trpc's jsonl
producer is written in.

The three positives cover the forms a generator is written in: an async
generator function, a sync one, and an async generator *method*. The single
intentional error is the last function, a plain function with a non-void return
type and no `return`, which still reports.
