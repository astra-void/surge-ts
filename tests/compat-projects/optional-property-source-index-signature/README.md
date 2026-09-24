# optional-property-source-index-signature

tsc's `propertiesRelatedTo` reads a source property with `getPropertyOfType`,
which never answers from an index signature: an optional target property the
source does not declare has nothing to relate, whatever the source's index
signature holds (next's `ParsedUrlQuery & { amp?: '1' }`), while a required
one is still missing (TS2741).
