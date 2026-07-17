import { Request, Response } from "webframe-core";
import "webframe";

function handler(req: Request, res: Response): void {
  const url: string = req.url;
  const framework: string = req.framework;
  const id: string | undefined = req.user?.id;
  const appName: string = req.app.name;
  req.app.handle(req, res);
  res.send(url + framework + (id ?? "") + appName);
  const bad: number = req.url;
  void bad;
}

export { handler };
