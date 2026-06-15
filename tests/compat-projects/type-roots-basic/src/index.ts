const value = process.env["TOKEN"];

const ok: string | undefined = value;
const bad: number = value;

export { ok, bad };
