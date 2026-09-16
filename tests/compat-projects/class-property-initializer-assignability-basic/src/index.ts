type Mode = "light" | "dark";

class Settings {
  count: number = "zero";
  static label: string = 1;
  readonly enabled: boolean = 0;
  private tags: string[] = [1];
  nested: { depth: number } = { depth: "deep" };
  mode: Mode = "ligth";
  format: (value: number) => string = (value) => value;
  valid: number = 1;
  inferred = 1;
  fromThis: number = this.valid;
  static fromStatic: string = Settings.label;
}
