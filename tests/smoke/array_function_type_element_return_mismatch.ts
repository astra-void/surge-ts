function listener(): string {
  return "ok";
}

let listeners: (() => void)[] = [listener];
