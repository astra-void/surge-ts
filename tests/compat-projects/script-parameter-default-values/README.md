# script-parameter-default-values

A script's top-level declarations are bound before any function signature is
resolved, so a hoisted function's parameter default reads them wherever they
are written (`known`). Only a name declared nowhere is TS2304.
