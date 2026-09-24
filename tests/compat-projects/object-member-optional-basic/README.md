# object-member-optional-basic

A `?` after an object-literal member's name is TS1162 at the `?`, from tsc's
grammar checks for property assignments, shorthand properties and methods.
The literal keeps its members, so the file goes on being checked.
