type Route<T extends string> = `/api/${T}`;
type UserRoute = Route<"users" | "me">;

const ok: UserRoute = "/api/users";
const bad: UserRoute = "/api/posts";
