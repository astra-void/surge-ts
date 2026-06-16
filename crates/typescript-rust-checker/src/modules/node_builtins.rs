//! Node built-in module specifier recognition.
//!
//! TypeScript does not bridge `node:fs` to `fs` (or vice versa): each
//! `declare module` is matched by exact specifier string, so a real
//! `@types/node` ships both `declare module "fs"` and `declare module
//! "node:fs"`. The only Node-protocol-specific behavior is the missing-module
//! diagnostic: when an unresolved specifier names a Node built-in (bare or
//! `node:`-prefixed), tsc reports the "install @types/node" hint (TS2580 /
//! TS2591) instead of the generic TS2307. This list mirrors tsc 6.0.3's
//! `nodeCoreModules` set so the TS2307-vs-hint boundary matches exactly.

const UNPREFIXED_NODE_CORE_MODULES: &[&str] = &[
    "assert",
    "assert/strict",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "dns/promises",
    "domain",
    "events",
    "fs",
    "fs/promises",
    "http",
    "http2",
    "https",
    "inspector",
    "inspector/promises",
    "module",
    "net",
    "os",
    "path",
    "path/posix",
    "path/win32",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "readline/promises",
    "repl",
    "stream",
    "stream/consumers",
    "stream/promises",
    "stream/web",
    "string_decoder",
    "sys",
    "timers",
    "timers/promises",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "util/types",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
];

const EXCLUSIVELY_PREFIXED_NODE_CORE_MODULES: &[&str] = &[
    "node:quic",
    "node:sea",
    "node:sqlite",
    "node:test",
    "node:test/reporters",
];

/// Whether `specifier` names a Node built-in module, matching tsc's
/// `nodeCoreModules` set (bare names, their `node:`-prefixed forms, and the
/// handful of names that exist only with the `node:` prefix).
pub(crate) fn is_node_core_module(specifier: &str) -> bool {
    if UNPREFIXED_NODE_CORE_MODULES.contains(&specifier) {
        return true;
    }

    if let Some(rest) = specifier.strip_prefix("node:")
        && UNPREFIXED_NODE_CORE_MODULES.contains(&rest)
    {
        return true;
    }

    EXCLUSIVELY_PREFIXED_NODE_CORE_MODULES.contains(&specifier)
}
