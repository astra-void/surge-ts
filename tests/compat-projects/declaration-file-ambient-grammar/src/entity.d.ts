declare namespace Values {
    const one: number;
}
declare module "entity-default" {
    export default Values.one;
}
declare module "entity-equals" {
    export = Values;
}
