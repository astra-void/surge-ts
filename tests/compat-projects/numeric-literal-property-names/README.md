# numeric-literal-property-names

A numeric literal names a property by its JavaScript `Number#toString` text
(tsc `getPropertyNameForPropertyNameNode`, `jsnum`): `0b11010` is `"26"`,
`1e1000` is `"Infinity"` (so the later `Infinity` key is a duplicate, TS1117),
an 84-digit binary literal is `"9.671406556917009e+24"`, `1e21` is `"1e+21"`
and `0.0000001` is `"1e-7"`. Element accesses by those strings find the
properties; an unknown key still reports TS7053.
