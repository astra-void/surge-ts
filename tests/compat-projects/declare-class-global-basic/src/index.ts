const headers = new FetchHeaders({ token: "x" });

const value = headers.get("token");
const ok: string | null = value;
const bad: number = value;

headers.set("token", "y");
headers.set("token", 123);

const made = FetchHeaders.from("raw");
const madeOk: FetchHeaders = made;
const madeBad: number = made;
