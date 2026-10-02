// @noImplicitOverride: true
// Implementing an abstract base member does not require `override`.
abstract class Base { abstract run(): number; } class Impl extends Base { run(): number { return 1; } }
