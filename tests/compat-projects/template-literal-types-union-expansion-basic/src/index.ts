type Entity = "users" | "posts";
type Action = "new" | "edit";
type Path = `/${Entity}/${Action}`;

const a: Path = "/users/new";
const b: Path = "/posts/edit";
const badEntity: Path = "/comments/new";
const badAction: Path = "/users/delete";
