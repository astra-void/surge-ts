class Base {
    get publicPrivate() { return 0; }
    private set publicPrivate(v: number) { }

    private get privatePublic() { return 0; }
    set privatePublic(v: number) { }
}

class Derived extends Base {
    run() {
        this.publicPrivate = 1;
        void this.publicPrivate;
        this.privatePublic = 1;
        this.privatePublic += 1;
        void this.privatePublic;
    }
}

declare const base: Base;
base.privatePublic += 1;
base.publicPrivate += 1;
base.publicPrivate = 2;
void base.publicPrivate;

export { Derived };
