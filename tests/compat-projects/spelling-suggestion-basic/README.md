# spelling-suggestion-basic

tsc turns `TS2304` into `TS2552` (`Did you mean 'X'?`) when an in-scope value name
is within its spelling threshold: an edit distance under `floor(len * 0.4)`, with
a case-only difference costing a tenth of an edit and candidates whose length
differs by more than `max(2, floor(len * 0.34))` skipped. surge suggested only a
case-insensitive exact match, and the call path (`defineNuxtConfig({})`) never
suggested at all. The unrelated name pins that `TS2304` survives.
