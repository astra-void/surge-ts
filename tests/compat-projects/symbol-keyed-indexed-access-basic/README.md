# symbol-keyed-indexed-access-basic

`Matchable[matcher]` — an indexed access whose key type is a symbol — reported
`TS2538 Type 'symbol' cannot be used as an index type`. The parser drops
computed members (`[matcher](): R` never enters the member table), so the report
was about surge's own gap rather than the source: there was nothing to validate
the key against. ts-pattern writes its matcher protocol exactly this way, and
`CustomP<…>[matcher]` sits in the type every pattern flows through.

A symbol key now degrades to the `unknown` sentinel without reporting, the same
way an open receiver and an unresolved key already do in that function. Modelling
symbol-named members properly is a separate, larger change; until then a silent
degrade is the honest answer.

`absentMemberStillReports` is the intentional error and bounds the suppression:
only the symbol *index* goes quiet, and an ordinary missing property on the same
interface still reports.
