class Base {
    x = 1;
    method() {
        return 1;
    }
}

declare const flag: boolean;

export class Branches extends Base {
    constructor(n: number) {
        if (flag) {
            super();
            this.x;
        } else {
            this.x;
            super.method();
        }
        this.x;
        switch (n) {
            case 1:
                super();
            case 2:
                this.x;
            default:
                super();
                this.x;
        }
        this.x;
    }
}

export class Both extends Base {
    constructor() {
        if (flag) {
            super();
        } else {
            super();
        }
        this.x;
    }
}

export class Conditional extends Base {
    constructor() {
        const a = { w: flag ? super() : 0 };
        this.x;
        const b = { w: flag ? super() : super() };
        this.x;
        const c = () => this.x;
        return;
    }
}

export class Logical extends Base {
    constructor() {
        flag && super();
        this.x;
    }
}

export class Loops extends Base {
    constructor() {
        while (flag) {
            super();
        }
        this.x;
        do {
            super();
        } while (flag);
        this.x;
    }
}

export class Parameters extends Base {
    constructor(y = this.x) {
        super();
    }
}

export class Thrown extends Base {
    constructor() {
        if (flag) {
            throw new Error();
        }
        super();
        this.x;
    }
}
