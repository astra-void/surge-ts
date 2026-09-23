# call-first-failing-argument

tsc relates a call's arguments in order and stops at the first that does not
fit (`getSignatureApplicabilityError`): only that argument's mismatch — or
its elaboration into an object literal, array literal or arrow — is
reported. Later arguments are still contextually typed and checked, so an
error of their own (the annotated `inner` inside the callback) is reported,
but their mismatches with the signature are not.
