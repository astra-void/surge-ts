# element-access-guard-narrowing-basic

tsc narrows an element access whose key is a literal or a `const` identifier
(`if (posts[0])`, `if (db.posts[index])`, `posts[index] !== undefined`,
`parts[1] && parts[1].length`), which matters under `noUncheckedIndexedAccess`
where every such read is `T | undefined`. An array has one element type for
every index, so this cannot be expressed by rewriting the binding's type; surge
remembers the guarded access under its rendered key instead. The two intentional
errors pin that an unguarded read and a *different* index still report.
