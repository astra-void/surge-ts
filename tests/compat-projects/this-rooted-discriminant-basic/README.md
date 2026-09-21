# this-rooted-discriminant-basic

`this.state.kind === "a"` narrows `this.state` exactly as `p.state.kind`
narrows `p.state`. The nested-discriminant narrower accepted only an
identifier as the root, so inside a class method (or a function with a `this`
parameter) the guarded member reads were false TS2339s. The discriminant
alias (`const kind = this.state.kind`) takes the same root.
