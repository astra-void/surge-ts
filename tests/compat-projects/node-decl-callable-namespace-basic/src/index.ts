import log = require("loglib");

log("hello");
const options: log.Options = { pretty: true };
const version: string = __APP_VERSION__;
const verbose: boolean = AppEnv.flags.verbose;
const badVersion: number = __APP_VERSION__;

export { options, version, verbose, badVersion };
