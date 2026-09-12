# namespace-reexport-augmentation-basic

fastify's entry file publishes `FastifyRequest` by re-exporting an *import*
from inside `declare namespace fastify { export { FastifyRequest } }` under
`export = fastify`, and `@fastify/websocket` augments it with
`declare module 'fastify' { interface FastifyRequest { ws: boolean } }`.

surge collected a namespace's members from its declarations only, so the
re-exported name never reached the module's export table under `fastify.X`;
the un-augmented import still resolved through a fallback, but the augmentation
was *inserted* as the only `FastifyRequest` entry and shadowed that fallback —
every declared member became a false `TS2339` while the augmentation's own
member kept working.

Imports live in the module's scope layers, not its local table, so the
re-exported member is published once the scope is assembled; the export-table
pass then carries it bare, and the augmentation merges into a real interface.
