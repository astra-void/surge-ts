interface Context {
  id: number;
}

type Handler = (this: Context, value: string) => void;

declare const handler: Handler;
