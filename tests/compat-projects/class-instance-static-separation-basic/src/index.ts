class User {
  id: string;
  static version: string;

  constructor(id: string) {
    this.id = id;
  }
}

const user = new User("a");

User.id;
user.version;
