# override-modifier-without-base-member-basic

An `override` modifier needs a base class that declares the member. A class
with no `extends` (implementing an interface does not count) reports every such
member as TS4112; a class whose base chain does not declare it reports TS4113
naming the direct base. surge reported neither.

TS4113 is decided only when every base in the chain is a class declared in
source, since a base surge cannot see may declare the member. Overrides of
members declared further up the chain, of accessors, and of abstract members are
pinned as clean.
