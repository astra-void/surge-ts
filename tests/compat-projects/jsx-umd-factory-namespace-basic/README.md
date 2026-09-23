# jsx-umd-factory-namespace-basic

A module that never imports React still reaches the UMD global `React`
(`export as namespace React`) as a namespace: tsc's `getJsxNamespaceAt`
resolves the factory with the namespace meaning, so the JSX namespace is the
module's `React.JSX`. Only the factory's value use is TS2686. The props are
then joined with `React.JSX.IntrinsicAttributes`, whose `key` is never
excess, while an attribute no constituent declares still is.
