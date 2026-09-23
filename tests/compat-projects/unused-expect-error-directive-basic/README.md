# unused-expect-error-directive-basic

An `@ts-expect-error` directive whose next non-blank, non-`//`-comment line has no error is TS2578 at the directive comment (the last line of a block comment); `@ts-ignore` never reports, and the directive must lead its comment (tsc's comment-directive processing).

Not in the default oracle sweep: TS2578 is opt-in, so compare it with `SURGE_REPORT_UNUSED_EXPECT_ERROR=1` set.
