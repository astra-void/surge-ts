export {};
const key = "k";
enum Named { ["literal"] = 1, [key] = 2, plain = 3 }
enum Twice { [key] = 1, [key] = 2 }
enum Template { [`text`] = 1 }
const wrong: string = Named.plain;
