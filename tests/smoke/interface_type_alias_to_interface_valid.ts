interface User {
  name: string;
}

type UserAlias = User;

let user: UserAlias = { name: "Ada" };
