// @target: es2022
class Base {
    field = 1;
    accessor auto = 2;
    method() { return 3; }
}
class Derived extends Base {
    read() {
        return super.field + super.auto + super.method();
    }
}
