interface User {
  id: string;
}

type Named = {
  name: string;
};

type UserProfile = User & Named;

const ok: UserProfile = { id: "u1", name: "Ada" };
const bad: UserProfile = { id: "u1" };
