interface Config {
  baseUrl?: string;
}

export function trim(config: Config): string {
  if (config.baseUrl?.endsWith("/")) {
    return config.baseUrl.substring(0, config.baseUrl.length - 1);
  }
  return "";
}
