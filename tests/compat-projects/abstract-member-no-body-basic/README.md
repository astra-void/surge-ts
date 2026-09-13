# abstract-member-no-body-basic

A bodyless class member — `abstract`, an overload signature, or a member of an
ambient class — was run through the function-body check, which then reported
`TS2355` for the missing return of the body that is not there. The identical
guard has always been on the function-declaration path
(`check_function_declaration`: "A bodyless `function` declaration is an ambient
declaration or an overload signature"); class members never had it.

`Cache` is drizzle-orm's shape and the two diagnostics this closed there.
`Overloaded` is the wider case: every class overload signature with a non-void
return reported, in any project.

`StillReportsARealMissingReturn` is the intentional error and pins the
direction: a method that *has* a body and never returns still reports. It also
pins the span, which moved with this fixture — tsc reports a missing return on
the written return type and falls back to the name, and a class method carried
no return-type span at all until now, so it always reported on the name.
