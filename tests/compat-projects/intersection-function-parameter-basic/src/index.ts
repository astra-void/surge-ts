type WithId = { id: string };
type WithRole = { role: "admin" | "user" };

function accept(value: WithId & WithRole) {
  const id: string = value.id;
  const role: "admin" | "user" = value.role;
}

accept({ id: "1", role: "admin" });
accept({ id: "1" });
accept({ id: "1", role: "guest" });
