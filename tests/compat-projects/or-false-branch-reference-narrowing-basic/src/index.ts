interface Program {
  check(): void;
}
type Services = { program?: Program; map?: number };

function earlyReturn(services: Services | undefined) {
  if (!services?.program || !services.map) {
    return;
  }
  services.program.check();
  services.map.toFixed();
}

function optionalOnBoth(services: Services | undefined) {
  if (!services?.program || !services?.map) {
    return;
  }
  services.program.check();
}

function withConstantOperand(services: Services | undefined) {
  if (!services?.program || false) {
    return;
  }
  services.program.check();
}

function stillOptional(services: Services | undefined) {
  if (!services?.map) {
    return;
  }
  services.program.check();
}
