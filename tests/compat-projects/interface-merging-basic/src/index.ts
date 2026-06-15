interface User {
  id: string;
}

interface User {
  name: string;
}

const ok: User = {
  id: "u1",
  name: "Ada",
};

const missing: User = {
  id: "u1",
};

const bad: User = {
  id: "u1",
  name: 123,
};
