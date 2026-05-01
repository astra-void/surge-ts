interface User {
  name: string;
}

type MaybeUser = User | undefined;

let value: MaybeUser = undefined;
