interface User {
  name: string;
  age?: number;
}

type Route = {
  path: string;
  secure?: boolean;
};

const okUser = { name: "Ada" } satisfies User;
const missingUser = {} satisfies User;
const wrongUser = { name: 123 } satisfies User;
const extraUser = { name: "Ada", unknown: true } satisfies User;

const okRoute = { path: "/" } satisfies Route;
const wrongRoute = { path: 1 } satisfies Route;

const okArray = [1, 2] satisfies Array<number>;
const wrongArray = [1, "x"] satisfies Array<number>;

const primitiveMismatch = 123 satisfies string;

const idMismatch = (1 as any as number) satisfies User;

function acceptUser(user: User): string {
  return user.name;
}

acceptUser({ name: "Ada" } satisfies User);
acceptUser({} satisfies User);

function makeUser(): User {
  return { name: "Ada" } satisfies User;
}

function makeWrongUser(): User {
  return { name: 123 } satisfies User;
}

const unresolved = { name: "Ada" } satisfies MissingType;

const literalLike = { mode: "dev" } satisfies { mode: string };
const mode: "dev" = literalLike.mode;
const badMode: "prod" = literalLike.mode;
