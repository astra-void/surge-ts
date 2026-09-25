declare const dec: any;

@dec(() => Deferred)
export class Deferred {}

@dec((() => Immediate)())
export class Immediate {}

@dec((function () {
    return Called;
})())
export class Called {}

export class Members {
    @dec((() => Members)()) static method() {}
    @dec(() => Members) instance() {}
}
