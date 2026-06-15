declare class MyRequest extends Request {
  constructor(input: string);
  get token(): string;
}

async function read(req: MyRequest) {
  const headers: Headers = req.headers;
  const body = await req.json();
  return { headers, body };
}

const req = new MyRequest("/api");

const okToken: string = req.token;
const badToken: number = req.token;
