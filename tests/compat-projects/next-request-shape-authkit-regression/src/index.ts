import { NextRequest } from "next/server";

async function handle(req: NextRequest) {
  const body = await req.json();
  const headers: Headers = req.headers;
  const cookies = req.cookies;
  const session = cookies.get("session");
  const url: string = req.nextUrl;

  const badCookies: number = req.cookies;

  return { body, headers, session, url, badCookies };
}
