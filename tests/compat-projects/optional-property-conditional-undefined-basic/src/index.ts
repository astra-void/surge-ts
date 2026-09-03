interface Props {
  cb?: () => void;
  n?: number;
  required: number;
}

declare function take(props: Props): void;
declare const flag: boolean;

take({ required: 1, cb: undefined });
take({ required: 1, cb: flag ? () => {} : undefined });
take({ required: 1, n: flag ? 1 : undefined });

const maybe: (() => void) | undefined = flag ? () => {} : undefined;
take({ required: 1, cb: maybe });

take({ required: undefined });

export { maybe };
