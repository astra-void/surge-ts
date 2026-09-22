# rest-tuple-parameters-basic

A rest parameter typed as a tuple expands into the positional parameters
the tuple spells out (`getExpandedParameters`), and an open tuple's fixed
elements keep their positions when indexed. surge compared the whole
tuple against every argument and read `t1[0]` as the union of everything
the tuple can hold.
