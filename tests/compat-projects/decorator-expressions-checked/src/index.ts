declare const decorate: any;

@missingClassDecorator class Decorated {}
@decorate(missingArgument) class WithArgument {}

class Members {
    @decorate(missingInMethod) method() {}
    @missingOnProperty property = 1;
    @decorate(missingOnStatic) static shared = 2;
    parameters(@decorate(parameterDecoratorIsNotChecked) value: number) { return value; }
}

@decorate(SelfReference) class SelfReference {}
@decorate(() => DeferredSelf) class DeferredSelf {}

class SelfInMember {
    @decorate(SelfInMember) static member() {}
    @decorate(() => SelfInMember) static deferred() {}
}

@((target, context) => {}) class ContextuallyTyped {}

@decorate(LaterClass) class BeforeLater {}
class LaterClass {}

declare function takesNothing(): (target: unknown, context: unknown) => void;
@takesNothing class CalledTooLate {}
@takesNothing() class CalledFirst {}

declare const factories: { make(): (target: unknown, context: unknown) => void };
@factories.make().extra class NeedsParentheses {}
@(factories.make()) class Parenthesized {}

@decorate var notDecoratable = 1;

@((first: unknown, second: unknown, third: unknown) => {}) class ExpectsTooMany {}
@(() => {}) class ExpectsNone {}
