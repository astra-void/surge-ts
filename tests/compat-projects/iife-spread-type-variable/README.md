# iife-spread-type-variable

An immediately invoked function takes its parameters' contextual types from
the call's arguments (tsc's `getContextuallyTypedParameterType`), a spread of
a generic body's type variable included: a rest parameter spread with `...t`
(`t: T`) is contextually typed, and so is a parameter before it, so none of
them is an implicit `any`.
