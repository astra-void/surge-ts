class Session {
  run(): void {}
}
class LocalSession extends Session {
  flush(): void {}
}
class Database {
  readonly session: Session = new Session();
  name = "db";
}
export class LocalDatabase extends Database {
  declare readonly session: LocalSession;
  name = "local";
}
