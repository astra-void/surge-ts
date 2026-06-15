class User {
  id: string;

  constructor(id: string) {
    this.id = id;
  }

  getId(): string {
    return this.id;
  }
}

const user = new User("a");

const okId: string = user.id;
const badId: number = user.id;

const okMethod: string = user.getId();
const badMethod: number = user.getId();
