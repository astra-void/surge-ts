import "webframe";

declare module "webframe-core" {
  interface Request {
    user?: { id: string };
  }
}
