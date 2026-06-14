type Config = {
  secret: string;
};

function getSecret(config: Config): string {
  return config["secret"];
}
