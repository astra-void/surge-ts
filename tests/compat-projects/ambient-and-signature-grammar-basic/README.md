# ambient-and-signature-grammar-basic

Grammar tsc checks without types: a statement in an ambient namespace or
module body (TS1036, once per block), `declare` inside one (TS1038), an index
signature key that is a literal or type parameter (TS1337) or a keyword other
than `string`/`number`/`symbol` (TS1268), two index signatures for one key type
(TS2374), an enum merged with anything but a namespace or enum (TS2567), and a
destructuring declaration with no initializer (TS1182).
