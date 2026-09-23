declare var target: any;

with (target) {
    namespace Inner { }
    export const hidden = 1;
    label: { }
}

namespace Outside { }
