import auth = require("pkg");

const token: string = auth.sign("payload");
void token;
