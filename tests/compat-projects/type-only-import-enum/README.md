# type-only-import-enum

An enum has a value, so a type-only import of one used as a value is TS1361
like a type-only class or namespace import (the type-only check runs where the
name resolves, before any not-found selection). An enum is not a namespace:
its `E.Member` types must not make its import read as a namespace alias, which
turned the use into TS2708. A plain import of an uninstantiated namespace used
as a value is still TS2708.
