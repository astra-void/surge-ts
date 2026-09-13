# predicate-property-path-subject-basic

A generic predicate whose subject is a *property path* narrowed nothing.
`predicate_type_argument_substitution` bailed outright on a non-empty path,
because the tested value is not the whole argument and the parameter annotation
cannot be matched against the subject's type.

That reasoning only covers the inference *from the subject*. A predicate whose
type parameters come from its other arguments — `is(options.role, Role)` takes
`T` from the class — has nothing to do with the subject, and dropping the whole
guard left every `is(x.y, C)` unnarrowed. Only the subject-based inference is
skipped now; a parameter still left unbound drops the guard as before, because
the `Any` fill is sound only while the predicate selects among the subject's own
union members, which needs the subject's type.

`fromAPropertyPath` and `fromAPropertyPathInAnIf` are the two forms.
`fromABindingStillWorks` is the control for the path that always worked.

`theOtherBranchIsTheOtherMember` is the intentional error and pins the
direction: the false branch really is the `string`, so reading `name` there
reports.
