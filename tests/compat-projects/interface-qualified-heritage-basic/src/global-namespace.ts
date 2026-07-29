export {};

declare global {
  namespace Express {
    interface Request {
      url: string;
    }
  }
}
