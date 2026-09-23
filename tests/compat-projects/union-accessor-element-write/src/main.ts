class One {
    get prop(): string { return ""; }
    set prop(value: string | number) {}
}
class Two {
    get prop(): string { return ""; }
    set prop(value: string | boolean) {}
}
declare const either: One | Two;

either["prop"] = 42;
either["prop"] = true;
either["prop"] = "text";
either["prop"] = null;

export {};
