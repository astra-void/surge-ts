# jsdoc-link-counts-as-use

tsc resolves the name in a JSDoc `{@link X}`, `{@linkcode X}` or
`{@linkplain X}` tag (`checkJSDocLinkLikeTag`), and that resolution counts as
a use of the import it names, so under noUnusedLocals `Linked` and `Coded`
are not unused. An import nothing refers to still is (TS6133).
