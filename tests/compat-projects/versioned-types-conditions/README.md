# versioned-types-conditions

tsgo matches an `exports` condition `types@<range>` (`IsApplicableVersionedTypesKey`)
and picks a `typesVersions` entry (`GetVersionPaths`) by testing an npm semver
range against the TypeScript version, 7.0.2. surge had no versioned `types@`
conditions at all and compared `typesVersions` keys against a pinned 6.0.

- `versioned/current` (`types@>=4`) and `versioned/ranged` (`types@^7.0.0`)
  resolve; `versioned/ancient` (`types@<4`) is TS2307.
- `legacy`'s `typesVersions` picks `">=7.0 <8"` over `"<7.0"`, so `fromOld`
  is TS2305.
