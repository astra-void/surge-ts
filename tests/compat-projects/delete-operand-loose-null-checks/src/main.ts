interface Box {
    required: number;
    optional?: number;
    readonly fixed: number;
}
declare const box: Box;

delete box.required;
delete box.optional;
delete box.fixed;
