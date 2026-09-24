# for-await-in-parse-error

`for await` takes only an `of` loop: tsc's parser expects `of` where
`for await (x in y)` has `in` (TS1005 at `in`). oxc reports the same failure in
its own words; surge classifies it as that parse error, so it gates the
program's semantic diagnostics like any other.
