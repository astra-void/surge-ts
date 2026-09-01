export const viaBlockArrow = () => {
    return Legacy.greet("arrow");
};

export function viaFunctionDeclaration() {
    return Legacy.greet("function");
}

export const viaUnresolvedName = () => {
    return MissingName;
};
