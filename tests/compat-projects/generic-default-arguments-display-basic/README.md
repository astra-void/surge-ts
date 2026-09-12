# generic-default-arguments-display-basic

A generic interface referenced without type arguments instantiates its
defaults, and tsc displays it with them filled in (`Box<number>`, not `Box`).
surge built the display name from the *written* arguments only, so an
argument-less reference had none — and, for a library class such as
`http.ServerResponse<Request = IncomingMessage>`, no display meant the
reference could not take the lazy path and was expanded eagerly, where a
heritage surge cannot resolve degraded the whole annotation to the sentinel.

The display is now rendered from the declaration's default types when no
arguments are written (interfaces and classes, and aliases that live in a
dependency's `.d.ts`; a user alias keeps its eager path). The two errors here
pin the rendered form; the aliased reads pin that the defaults bind.
