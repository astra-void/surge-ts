# jsx-tag-signature-checks

tsc resolves a value tag like a call (`resolveJsxOpeningLikeElement`). A tag
whose type has nothing to call — a number, an object without construct or
call signatures — is TS2604. A tag whose value is a string literal is the
intrinsic element it names; one `IntrinsicElements` lacks is TS2339 at the
element and TS2604 at the tag. Without `JSX.ElementType`, what a function
component returns must be an `Element` or `null` and what a class component
constructs an `ElementClass` (`checkJsxReturnAssignableToAppropriateBound`,
TS2786). A class instance without the `ElementAttributesProperty` member
takes no attributes (TS2607).
