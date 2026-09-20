# method-signature-implicit-return-basic

A method signature written without a return type (`apply(blah: any);`)
returns an implicit `any`. surge's parser dropped the whole member, so the
interface required nothing and every read of the method was a missing
property. A value that would fit once constructed (`needs = Holder`) is
reported on the value, as `elaborateDidYouMeanToCallOrConstruct` does.
