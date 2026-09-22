# cannot-find-name-lib-basic

The lib- and `types`-dependent half of tsc's cannot-find-name messages: under
`lib: ["ES5"]` a newer global is TS2583 (naming the lib that declares it) or,
when ES5 declares only its type, TS2585; `console`/`document` without the DOM
lib are TS2584. With `"types": ["*"]` the node, test-runner, jQuery and Bun
hints drop their "add it to the types field" tail (TS2580, TS2582, TS2581,
TS2867).
