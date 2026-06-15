const user = new User();

const okId: string = user.id;
const jar: CookieJar = user.cookies;
const cookie: string | undefined = jar.get("session");

const badId: number = user.id;
const badJar: number = user.cookies;
