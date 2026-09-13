# grammar-parameter-list-hardening

Four parameter-list rules that need no types and that surge did not have:

- a parameter written both `?` and with a default (`TS1015`),
- a required parameter after an optional one (`TS1016`), in a function and in
  an arrow,
- a default on a signature with no body to run it (`TS2371`),
- a parameter property on a constructor signature (`TS2369`).

`defaultedThenRequired` is the distinction the TS1016 rule turns on: a
*defaulted* parameter does not make the next one illegal, only a `?` one does.
`optionalThenRest` and `Implementation` pin the other two non-reports.
