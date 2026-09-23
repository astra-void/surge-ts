# tuple-rest-target-positions-basic

A tuple is related to a tuple target that has a rest element position by
position (tsc's `propertiesRelatedTo`): a source without a rest element must
reach the target's `minLength`, and each source element is related to the
target element it lands on — counted from the start within the target's
leading elements, from the end past them. A target element the source may
lack (a rest element, or an optional one) must not land on a required one.

surge compared the target's leading elements as if all were required, so
`[string]` and `[string, ...number[]]` were rejected for
`[string, number?, ...number[]]`, whose second element is optional.

The errors from `optionalIntoRest` down are intentional and are `tsc` errors
too.
