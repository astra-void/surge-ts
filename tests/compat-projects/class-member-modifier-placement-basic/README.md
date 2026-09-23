# class-member-modifier-placement-basic

`checkGrammarModifiers` placement rules oxc does not carry: `accessor` on
anything but a property declaration (TS1275), `abstract` on anything but a
class, method or property (TS1242) — a constructor, a parameter, an enum, an
interface, a namespace, a variable — and a parameter-property modifier on a
rest parameter (TS1317). `checkPropertyDeclaration` rejects an initializer on
an abstract property (TS1267), `checkGrammarProperty` a field literally named
`"constructor"` (TS18006), and `checkParameter` a `this` parameter that is not
first (TS2680) or an optional binding-pattern parameter in an implementation
(TS2463).

The TS1317 and TS2680 cases sit in their own files: oxc fails to parse a
modifier before `...` or a `this` after another parameter and gives up on the
file there, so nothing else in it would be reported.
