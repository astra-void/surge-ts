class User {
  id: string;
  static version: string;

  constructor(id: string) {
    this.id = id;
  }

  static create(id: string): User {
    return new User(id);
  }
}

const okVersion: string = User.version;
const badVersion: number = User.version;

const okUser: User = User.create("a");
const badUser: number = User.create("a");
const badArg = User.create(123);
