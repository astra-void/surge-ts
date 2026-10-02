// @strict: true
interface LocalPlugin {
  configure(value: "serve" | "build"): void;
  buildStart(): "build" | undefined;
}

function makePlugin(): LocalPlugin {
  let command: "serve" | "build" = "serve";
  return {
    configure(value) {
      command = value;
    },
    buildStart() {
      if (command !== "build") return;
      return command;
    },
  };
}
