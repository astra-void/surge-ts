abstract class C {
  private abstract m(): void;
  static abstract n(): void;
  async abstract o(): Promise<void>;
  abstract async p(): Promise<void>;
  accessor readonly a = 1;
  readonly accessor b = 1;
  declare override x: number;
  protected abstract q(): void;
  static async r() {}
}
export {};
