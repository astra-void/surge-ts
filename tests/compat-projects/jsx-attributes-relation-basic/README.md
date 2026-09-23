# jsx-attributes-relation-basic

tsc relates a JSX element's attributes object to the props of the signature it
resolves (`checkApplicableSignatureForJsxCallLikeElement`) and reports the
first failure the relation finds: every attribute whose value does not fit its
prop (`elaborateJsxComponents`), otherwise the first excess attribute,
otherwise the missing props (TS2741, or TS2739 for several). An excess
attribute is therefore the only report even when a required prop is missing
too, and a mismatched value hides both.

A function component's props are its first parameter (`unknown` without one)
joined with `JSX.IntrinsicAttributes`; a class component's are the instance's
`ElementAttributesProperty` member. An intrinsic element's props are not
joined. Hyphenated attributes are never excess. With several signatures each
is tried in turn, and when none fits the last one's error is TS2769.
