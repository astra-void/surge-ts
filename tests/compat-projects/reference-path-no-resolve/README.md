# reference-path-no-resolve

Under `noResolve` tsgo's file loader skips every file's reference directives
(`filesparser.go`): a `/// <reference path>` neither adds the file it names —
so what that file declares is not in scope — nor reports one it cannot find.
