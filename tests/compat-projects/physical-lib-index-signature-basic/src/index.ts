interface Env {
  [key: string]: string | undefined;
}

const env: Env = {};
const value = env["TOKEN"];

const ok: string | undefined = value;
const bad: number = value;
