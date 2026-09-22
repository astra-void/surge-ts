# object-signature-relation-basic

`signaturesRelatedTo`: every call and construct signature a target object
type declares has to be matched by one of the source's, and a source with
none matches nothing. surge related two object types by their properties
and index signatures only, so a constructor type was assignable to any
other, a class to a constructor type it cannot satisfy, and a plain
object to a callable interface. A type that is nothing but one signature
also prints as that signature rather than `{}`.
