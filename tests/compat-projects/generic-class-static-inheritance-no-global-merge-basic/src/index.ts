import { Base } from "./base";

export class MutationObserver<TValue> extends Base {
  constructor(
    public client: string,
    public options: TValue,
  ) {
    super();
  }
}

export const observer = new MutationObserver("client", { retry: true });
