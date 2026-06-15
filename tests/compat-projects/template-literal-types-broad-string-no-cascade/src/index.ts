type AnyId = `id:${string}`;

const ok: AnyId = "id:anything";
const bad: AnyId = "name:anything";
