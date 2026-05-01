interface User {
  name: string;
}

type MaybeUser = User | undefined;

let value: MaybeUser = { name: 123 };
