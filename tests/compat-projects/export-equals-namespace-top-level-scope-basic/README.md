# export-equals-namespace-top-level-scope-basic

A declaration outside every namespace resolves its body in its own file's
top-level scope. `@types/react` declares `type NativeSubmitEvent = SubmitEvent`
at the top of the module, beside `declare namespace React { interface
SubmitEvent … }`: the alias names the DOM's global `SubmitEvent`. surge resolved
it while a namespace member that referenced it was being resolved, still under
that namespace's prefix, so every `Native*` alias named React's own interface
and a `React.FormEvent` handler no longer fit `onSubmit`.
