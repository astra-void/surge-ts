# object-to-array-missing-members-basic

An object that is not an array lacks the array's members, and tsc says so:
`Type 'Shape' is missing the following properties from type 'Shape[]': length,
pop, push, concat, and N more` (TS2740) rather than a bare TS2322. The count
follows surge's own table of array members, not the configured lib's, so only
the message text can differ.
