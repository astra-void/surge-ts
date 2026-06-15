type Props = { id: string; className: string; title: string };

export function pick({ className, ...rest }: Props) {
  return { className, rest };
}

const grab = ({ id, ...others }: Props) => others;

const source = { a: 1, b: 2, c: 3 };
const { a, ...remaining } = source;

export { grab, a, remaining };
