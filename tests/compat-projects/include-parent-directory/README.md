# include-parent-directory

An `include` that climbs out of the config directory (`../shared/**/*.ts`) is
walked from its own base (tsc's `getBasePaths`), with `exclude` rebased the
same way and wildcards still skipping dot-directories.
