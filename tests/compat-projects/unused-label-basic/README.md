# unused-label-basic

A label no `break`/`continue` targets is TS7028 when `allowUnusedLabels` is
explicitly `false` (the binder marks it, `checkLabeledStatement` reports).
A jump inside a nested function or arrow starts a fresh label list, and a
nested label of the same name takes the jumps under it.
