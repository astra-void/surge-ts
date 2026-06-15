type A = `user:${"id"}`;

const ok: A = "user:id";
const bad: A = "user:name";
