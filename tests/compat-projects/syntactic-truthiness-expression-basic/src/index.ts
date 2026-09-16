declare const text: string;

// Syntax that is always truthy or always falsy is reported wherever it is tested.
if ({}) {}
if ([]) {}
if (() => text) {}
if (/pattern/) {}
if ("") {}
if ("value") {}
if (null) {}
if (undefined) {}
if (void 0) {}
while ([]) {
  break;
}
const negated = !"";
const left = {} && text;
const either = text || {};
const choice = "value" ? 1 : 2;
if (`template`) {}
if (text ? {} : []) {}

function body(flag: boolean): void {
  do {} while (null);
  for (; ({}); ) {
    break;
  }
  if ((({}) as unknown)) {}
}

// Idioms and non-literal syntax are left alone.
if (0) {}
if (1) {}
if (0.0) {}
if (true) {}
if (`template ${text}`) {}
if (text ? {} : "") {}
if (new Date()) {}
