export class CtorA { constructor<T>() {} }
export class CtorB { constructor(): void {} }
export class CtorC { constructor(this: CtorC) {} }
