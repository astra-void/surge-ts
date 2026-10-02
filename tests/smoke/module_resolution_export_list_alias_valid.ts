// @filename: src/a.ts
type UserId = string; export { UserId as UserModel };
// @filename: src/b.ts
import { UserModel } from "./a"; let id: UserModel = "u1";
