# script-namespace-qualified-members

A qualified type name in a global script resolves through the namespace's
exports as it does in a module (`resolveEntityName`): a member the namespace
does not export is TS2694, including across the blocks of a namespace merged
from several script files, a dotted `namespace M.P`, and an ambient namespace,
all of whose members are exported.
