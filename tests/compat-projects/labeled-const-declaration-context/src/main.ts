declare const flag: boolean;

if (flag) {
    inner: const allowed = 0;
}

outer: middle: const alsoAllowed = 1;

if (flag) label: const disallowed = 2;

while (flag) first: second: const alsoDisallowed = 3;

export { };
