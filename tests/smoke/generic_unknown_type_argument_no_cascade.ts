type Box<T> = { value: T };

let box: Box<Missing> = { value: 123 };
