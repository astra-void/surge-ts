# overloaded-signature-infer-return-basic

Every `infer` in a conditional's `extends` clause declares a type parameter of
the conditional (tsc's binder collects them into its locals), wherever it sits.
surge folds a type literal's overloaded call signatures into one signature that
keeps a single return type, so a capture in a later overload's return
(`infer Sr2`) was never declared: when the check type degraded and the true
branch was taken without binding the captures, `Sr2` reported TS2304.
