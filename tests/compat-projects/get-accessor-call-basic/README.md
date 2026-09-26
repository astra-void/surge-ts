# get-accessor-call-basic

Calling a `get` accessor with no arguments reports TS6234 instead of TS2349 (`invocationErrorDetails`): the member the read resolves to decides, so an inherited getter counts and a property that overrides it does not. A call with arguments, a set-only accessor, or a method is not affected.
