# nested-destructuring-assignment-targets

A destructuring assignment assigns every target it contains (tsc's
`bindDestructuringTargetFlow`): a nested object or array pattern reads its own
source's property or element, and a rest target the remaining elements or
properties. Reading any of them afterwards is not TS2454; each target is still
checked against its declared type (`g` is TS2322), an object rest as the source
without the named properties, and a local never assigned is still TS2454.
