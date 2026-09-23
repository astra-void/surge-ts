const typed = new Int8Array(4);
typed.at(0);
new Float64Array(2).at(-1);
new BigUint64Array(1).at(0);

const error = new Error("outer");
error.cause;

[1, 2, 3].at(0);
"text".at(0);

typed.notAMember;
error.notAMember;
