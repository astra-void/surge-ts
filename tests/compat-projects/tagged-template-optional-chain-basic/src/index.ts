declare const a: { b: (s: TemplateStringsArray) => void };
a?.b`x`;
a?.b?.`y`;
(a?.b)`z`;
a.b`w`;
export {};
