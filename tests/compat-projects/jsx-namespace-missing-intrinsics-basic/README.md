# jsx-namespace-missing-intrinsics-basic

With no `IntrinsicElements` in the JSX namespace tsc picks, an intrinsic
element is an implicit `any` (TS7026) at its opening and at its closing tag.
`noNamespace.tsx` compiles to `h`, which has no `JSX` and no global `JSX`
exists; `noIntrinsics.tsx` compiles to `bare`, whose `JSX` declares only
`Element`.
