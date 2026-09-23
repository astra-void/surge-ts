# lazy-export-nested-reference-basic

A module value the binding passes cannot type yet (`derived` re-exports an
import) is typed when the check phase first reads it. Two exposures of that:
the answer's nested references (`Decorate<V>` for `todo`) were interned into
the collector's private store and peeled to the sentinel once it was dropped,
so `client.todo.missing` went unreported; and an *annotated* export whose
annotation reads such a value (`typeof derived`) was frozen at the sentinel.
