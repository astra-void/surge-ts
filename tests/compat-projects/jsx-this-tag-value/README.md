# jsx-this-tag-value

tsc's `isJsxIntrinsicTagName` takes only a lowercase identifier (or one with a
`-`) or a namespaced name as an intrinsic tag, so `<this.tagName>` is a value
reference read off `this`, not an intrinsic element: with no
`JSX.IntrinsicElements` it is not TS7026, while the lowercase `<span />` is.
