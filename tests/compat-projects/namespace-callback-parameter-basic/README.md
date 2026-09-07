# namespace-callback-parameter-basic

A namespace value object models only the member *set*, so every member reads as
a permissive `(...args: any[]) => any`. Members whose real signature is needed
at the call site are published separately under a qualified `ns.member` key —
but only generic ones and type predicates were, because publishing every member
of every ambient namespace measured +300 MB peak RSS on tRPC.

A member that takes a **callback** needs its signature for the same reason a
type predicate does: without it the callback's own parameters have no
contextual type and become a false implicit-any. That is `ts.findConfigFile(dir,
(fileName) => …)`, which this fixture models. Only the *written* parameter form
is consulted, so the extra publishing stays proportional to the callback-taking
members rather than to all of them.

`versionLength` pins that a plain non-generic member still resolves through the
namespace object. The single intentional error is the last function: the
callback parameter is now really typed `string`, so a bogus member access on it
is reported.
