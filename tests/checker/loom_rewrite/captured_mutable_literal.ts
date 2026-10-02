// @strict: true
function makePlugin() {
  let command: "serve" | "build" = "serve";
  return {
    configure(value: "serve" | "build") {
      command = value;
    },
    buildStart() {
      if (command !== "build") return;
      return command;
    },
  };
}
