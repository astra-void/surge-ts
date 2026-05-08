interface User {
  id: string;
  email: string;
}

interface AdapterUser extends User {
  role?: string;
}

interface Broken extends MissingBase {}

const ok: AdapterUser = { id: "1", email: "a@example.com" };
const missingBase: AdapterUser = { id: "1" };
const mismatchBase: AdapterUser = { id: 1, email: "a@example.com" };
const broken: Broken = {};

function read(user: AdapterUser): string {
  return user.email;
}
