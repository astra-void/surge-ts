class User {
  id: string;
  constructor(id: string) {
    this.id = id;
  }
}

interface User {
  name: string;
}

const user = new User("u1");
user.name;
