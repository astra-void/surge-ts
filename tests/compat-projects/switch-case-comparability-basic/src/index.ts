enum Color {
  Red,
  Blue,
}

export function byString(value: string): void {
  switch (value) {
    case missingName:
      break;
    case 1:
      break;
    case 'ok':
      break;
  }
}

export function byUnion(value: 'a' | 'b'): void {
  switch (value) {
    case 'c':
      break;
    case 'a':
      break;
  }
}

export function byEnum(color: Color): void {
  switch (color) {
    case Color.Red:
      break;
    case Color.Blue:
      break;
  }
}

export function byLocal(value: number): void {
  const limit = 3;
  switch (value) {
    case limit:
      break;
    default:
      break;
  }
}
