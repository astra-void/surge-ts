# class-static-block-body-basic

A class `static { … }` block is code run with `this` bound to the class.
surge dropped static blocks while lowering the class, so nothing inside one
was ever checked.
