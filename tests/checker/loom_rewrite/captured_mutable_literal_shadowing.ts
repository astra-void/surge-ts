// @strict: true
function parameterShadow() {
  let command: "serve" | "build" = "serve";
  return {
    configure(command: "serve" | "build") {
      command = "build";
      return command;
    },
    read(): "serve" {
      return command;
    },
  };
}

function eagerRead(): { current: "serve"; configure(value: "serve" | "build"): void } {
  let command: "serve" | "build" = "serve";
  return {
    current: command,
    configure(value) {
      command = value;
    },
  };
}

function blockShadow() {
  let command: "serve" | "build" = "serve";
  return {
    configure() {
      {
        let command: "serve" | "build" = "serve";
        command = "build";
        void command;
      }
    },
    read(): "serve" {
      return command;
    },
  };
}
