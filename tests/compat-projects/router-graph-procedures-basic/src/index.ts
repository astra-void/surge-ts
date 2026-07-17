import { procedure, router, InputOf, OutputOf } from "./rpc";

interface User {
  id: string;
  name: string;
}

const userRouter = router({
  get: procedure<{ id: string }, User>(),
  list: procedure<{ limit: number }, User[]>(),
});

const app = router({
  user: userRouter,
  health: procedure<void, "ok">(),
});

type GetInput = InputOf<typeof app.user.get>;
type GetOutput = OutputOf<typeof app.user.get>;

type RouterFlags<T> = { [K in keyof T]: boolean };
const flags: RouterFlags<typeof userRouter> = {
  get: true,
  list: false,
  _router: true,
};

const input: GetInput = { id: "1" };
const pendingUser: Promise<User> = app.user.get(input);
const pendingUsers: Promise<User[]> = app.user.list({ limit: 10 });
const pendingHealth: Promise<"ok"> = app.health();
const output: GetOutput = { id: "1", name: "a" };

const badInput = app.user.get({ id: 42 });

void flags;
void pendingUser;
void pendingUsers;
void pendingHealth;
void output;
void badInput;
