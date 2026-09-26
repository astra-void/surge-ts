# jsx-fragment-factory-pragma

A file with an `@jsx` pragma and no `@jsxFrag` pragma cannot use fragments
under a JSX transform: TS17017 at each fragment (`checkJsxFragment`). With
both pragmas the fragment is fine.
