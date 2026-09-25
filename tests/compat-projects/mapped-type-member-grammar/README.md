# mapped-type-member-grammar

A mapped type may not declare members beside its own (TS7061, at the first of
them), and neither may a type literal, interface or class with a member named
like a mapped type's key (`[P in Keys]: T`), which tsc reports at the first
member of its container (`checkGrammarMappedType`, `checkGrammarProperty`).
tsc's parser accepts all of these, so the file is bound and checked even
where oxc gives up on it.
