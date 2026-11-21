export function required(name: string): string {
  const value = process.env[name];
  if (!value?.trim()) throw new Error(`Set ${name} for the isolated live-test environment.`);
  return value;
}

export function origin(name: string): string {
  const url = new URL(required(name));
  if (
    !['http:', 'https:'].includes(url.protocol) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    url.pathname !== '/'
  ) {
    throw new Error(`${name} must be an HTTP(S) origin without credentials or a path.`);
  }
  return url.origin;
}

export function settings() {
  return {
    studio: origin('DONKA_E2E_BASE_URL'),
    runtime: origin('DONKA_E2E_RUNTIME_URL'),
    mailpit: origin('DONKA_E2E_MAILPIT_URL'),
    email: required('DONKA_E2E_ADMIN_EMAIL'),
    password: required('DONKA_E2E_ADMIN_PASSWORD'),
  };
}
