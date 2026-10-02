class User { id: string; constructor(id: string) { this.id = id; } }
const ok = new User("a");
const okId: string = ok.id;
const bad = new User(123);
