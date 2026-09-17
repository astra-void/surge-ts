# class-accessor-bodies-basic

A class `get`/`set` accessor body is checked like a method body. surge
lowered accessors to their types only and never checked either body; a
setter parameter written without a type takes the getter's annotation, as
tsc infers it.
