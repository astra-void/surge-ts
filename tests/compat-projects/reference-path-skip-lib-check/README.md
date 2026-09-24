# reference-path-skip-lib-check

A missing `/// <reference path>` is one of the referencing file's semantic
diagnostics, so `skipLibCheck` drops a declaration file's (tsgo's
`SkipTypeChecking`) while a source file's TS6053 stays.
