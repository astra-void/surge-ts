interface Events {
  click: unknown;
  close: unknown;
}

type HandlerName = `on${keyof Events}`;

const ok: HandlerName = "onclick";
const bad: HandlerName = "onsubmit";
