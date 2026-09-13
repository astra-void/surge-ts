# grammar-multiple-default-exports-basic

A module with two default exports is `TS2528` **twice** — tsc reports each of
them. surge previously reported this as its own `surge::duplicate-default-export`,
once, on the second one only.

The second default here is written as `export { second as default }`, which is
the spelling the module-analysis path saw differently from `export default`.
