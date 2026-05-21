import { AdapterUser } from "./adapter";

function mapUser(user: { id: string; email: string }): AdapterUser {
  return {
    id: user.id,
    email: user.email,
    role: "user",
  };
}

const missing: AdapterUser = { role: "user" };
