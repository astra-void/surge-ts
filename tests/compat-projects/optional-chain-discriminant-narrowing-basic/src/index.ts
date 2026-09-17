interface Circle {
  kind: "circle";
  radius: number;
}
interface Square {
  kind: "square";
  side: number;
}

declare const maybeShape: Circle | Square | null;
if (maybeShape?.kind === "circle") {
  const radius: string = maybeShape.radius;
}

declare const optionalShape: Circle | Square | undefined;
if (optionalShape?.kind !== "square") {
  optionalShape.radius;
}
