# element-access-missing-key-selection

tsc's `getPropertyTypeForIndexType` picks the diagnostic for a literal key the
receiver lacks under `noImplicitAny`: on an object literal written in place it
is a missing property (TS2339); a receiver whose own `get` method takes the key
was probably meant to be called (TS7052, spelled from the receiver's access
path, `getSuggestionForNonexistentIndexSignature`); otherwise the access is an
implicit `any` (TS7053). A variable holding an object literal has the widened
type and takes the TS7053 path.
