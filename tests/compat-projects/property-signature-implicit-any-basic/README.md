# property-signature-implicit-any-basic

A property signature written without a type (`key;`) is an implicit `any`
member (TS7008). The parser dropped it, so every read of it was a false
TS2339 and the interface required nothing — a number was assignable to it.
