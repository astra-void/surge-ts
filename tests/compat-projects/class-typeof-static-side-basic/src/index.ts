class User {
  id: string;
  static version: string;

  constructor(id: string) {
    this.id = id;
  }
}

type UserCtor = typeof User;

const ctor: UserCtor = User;
const ok: string = ctor.version;
const bad: number = ctor.version;
