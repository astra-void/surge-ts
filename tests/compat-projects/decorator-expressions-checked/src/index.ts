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
