# import-member-suggestion-basic

tsc's `errorNoModuleMemberSymbol` for a named import the module does not
export: the closest exported name is TS2724, a module with a default export
gets TS2614 ("use `import x from`"), and only otherwise TS2305. surge reported
TS2305 for all of them.
