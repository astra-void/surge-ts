# import-equals-entity-alias-basic

`import x = N.M` aliases an entity name: the alias carries every meaning of
the entity (value, type, namespace), resolved from the declaring block
(`getTargetOfImportEqualsDeclaration`), and an alias may name another alias.
It is scoped like any block declaration and hidden by a nearer declaration of
the same name only for the meaning that declaration has: a parameter `C` hides
the value `C` but not the type `C`, while a type parameter `C` hides the type.
