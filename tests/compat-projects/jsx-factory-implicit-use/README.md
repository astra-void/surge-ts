# jsx-factory-implicit-use

Under `jsx: react`, a JSX element reads its factory and a fragment reads its
fragment factory (tsc `markJsxAliasReferenced`). The `/** @jsx h */` and
`/** @jsxFrag Fragment */` pragmas name them; without pragmas the factory is
`React.createElement`, rooted at `jsxFactory` or `reactNamespace` when set.
Those imports are used under `noUnusedLocals`, while `Fragment` in a file with
elements but no fragment stays unused (TS6133).
