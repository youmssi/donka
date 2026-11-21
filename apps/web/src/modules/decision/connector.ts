import { z } from 'zod';

/**
 * Connector nodes: a decision calls an outside service (a credit bureau, a KYC
 * or AML provider). The node is a custom node of kind `donka.connector`; its
 * `config` is read by the same handler in Studio (mock responses only) and in
 * the Runtime (real calls). The rules below mirror that handler's checks
 * (`donka-connectors`), so a node the form accepts is a node both accept.
 */
export const CONNECTOR_KIND = 'donka.connector';
export const SECRET_ENV_PREFIX = 'DONKA_SECRET_';
export const SECRET_NAME_MAX = 64;
/** What the Runtime allows at most by default; it caps anything higher. */
export const TIMEOUT_MAX = 10_000;
export const RETRIES_MAX = 3;

export const PRESETS = ['http', 'bureau-score'] as const;
export type Preset = (typeof PRESETS)[number];
export const AUTH_TYPES = ['none', 'bearer', 'header', 'oauth2'] as const;
export const CLIENT_ID_MAX = 200;
export const SCOPES_MAX = 20;
export const ON_ERROR = ['fail', 'fallback'] as const;

type Auth =
  | { type: 'none' }
  | { type: 'bearer'; secret: string }
  | { type: 'header'; header: string; secret: string }
  /** OAuth2 client credentials: `secret` names the client secret. */
  | { type: 'oauth2'; tokenUrl: string; clientId: string; secret: string; scopes?: string[] };

/** Mutual TLS: the names of the secrets holding the client certificate, its key and the CA. */
export interface ClientTls {
  certificate: string;
  key: string;
  ca?: string;
}

/** A connector node's settings, as the decision stores them. Never a secret value. */
export interface ConnectorConfig {
  preset?: Preset;
  url: string;
  auth?: Auth;
  tls?: ClientTls;
  body?: unknown;
  outputKey: string;
  timeoutMs?: number;
  retries?: number;
  onError?: (typeof ON_ERROR)[number];
  fallback?: unknown;
  mock?: unknown;
}

/** `BUREAU_API_KEY`: capital letters, digits and underscores, starting with a letter. */
export function validSecretName(name: string): boolean {
  return new RegExp(`^[A-Z][A-Z0-9_]{0,${SECRET_NAME_MAX - 1}}$`).test(name);
}

/** Scopes as the form holds them: words separated by spaces. */
export function scopeWords(text: string): string[] {
  return text.split(/\s+/).filter(Boolean);
}

// RFC 6749 §3.3: printable ASCII except space, double quote and backslash.
const SCOPE = /^[!#-[\]-~]+$/;

/** Text that parses as JSON (any value). */
function parsesAsJson(text: string): boolean {
  try {
    JSON.parse(text);
    return true;
  } catch {
    return false;
  }
}

const json = z.string().refine(parsesAsJson, 'json');
const wholeNumber = (max: number) =>
  z
    .string()
    .trim()
    .refine((text) => text === '' || (/^\d+$/.test(text) && Number(text) <= max), 'wholeNumberMax');

/** The settings as the form edits them: JSON as text, numbers as text. */
export const connectorSchema = z
  .object({
    preset: z.enum(PRESETS),
    url: z
      .string()
      .trim()
      .regex(/^https?:\/\/\S+$/, 'connectorUrl'),
    authType: z.enum(AUTH_TYPES),
    header: z.string().trim(),
    secret: z.string().trim(),
    tokenUrl: z.string().trim(),
    clientId: z.string().trim(),
    scopes: z.string(),
    tls: z.boolean(),
    certificate: z.string().trim(),
    key: z.string().trim(),
    ca: z.string().trim(),
    body: json,
    outputKey: z
      .string()
      .trim()
      .regex(/^[A-Za-z0-9_]+$/, 'outputKey'),
    timeoutMs: wholeNumber(TIMEOUT_MAX),
    retries: wholeNumber(RETRIES_MAX),
    onError: z.enum(ON_ERROR),
    fallback: z.string(),
    mock: json,
  })
  // Fields that depend on others; checked even while other fields are still wrong,
  // so every problem shows at once.
  .superRefine(
    (values, ctx) => {
      if (values.authType !== 'none' && !validSecretName(values.secret)) {
        ctx.addIssue({ code: 'custom', path: ['secret'], message: 'secretName' });
      }
      if (values.authType === 'header' && !/^[A-Za-z0-9-]+$/.test(values.header)) {
        ctx.addIssue({ code: 'custom', path: ['header'], message: 'headerName' });
      }
      if (values.authType === 'oauth2') {
        if (!/^https?:\/\/\S+$/.test(values.tokenUrl)) {
          ctx.addIssue({ code: 'custom', path: ['tokenUrl'], message: 'connectorUrl' });
        }
        if (
          !values.clientId ||
          values.clientId.length > CLIENT_ID_MAX ||
          /[\u0000-\u001f\u007f]/.test(values.clientId)
        ) {
          ctx.addIssue({ code: 'custom', path: ['clientId'], message: 'clientId' });
        }
        const scopes = scopeWords(values.scopes);
        if (scopes.length > SCOPES_MAX || !scopes.every((scope) => SCOPE.test(scope))) {
          ctx.addIssue({ code: 'custom', path: ['scopes'], message: 'scopes' });
        }
      }
      if (values.tls) {
        for (const name of ['certificate', 'key'] as const) {
          if (!validSecretName(values[name])) ctx.addIssue({ code: 'custom', path: [name], message: 'secretName' });
        }
        if (values.ca && !validSecretName(values.ca)) {
          ctx.addIssue({ code: 'custom', path: ['ca'], message: 'secretName' });
        }
      }
      if (values.onError === 'fallback' && !parsesAsJson(values.fallback)) {
        ctx.addIssue({ code: 'custom', path: ['fallback'], message: 'json' });
      }
    },
    { when: ({ value }) => typeof value === 'object' && value !== null },
  );
export type ConnectorValues = z.infer<typeof connectorSchema>;

const indent = (value: unknown) => JSON.stringify(value, null, 2);

/** What a preset fills in; the URL stays the author's. */
export function presetValues(preset: Preset): Omit<ConnectorValues, 'url'> {
  const common = {
    preset,
    timeoutMs: '',
    retries: '',
    onError: 'fail' as const,
    fallback: '',
    tokenUrl: '',
    clientId: '',
    scopes: '',
    tls: false,
    certificate: '',
    key: '',
    ca: '',
  };
  if (preset === 'bureau-score') {
    return {
      ...common,
      authType: 'header',
      header: 'X-Api-Key',
      secret: 'BUREAU_API_KEY',
      body: indent({ nationalId: '{{ applicant.nationalId }}' }),
      outputKey: 'bureau',
      onError: 'fallback',
      fallback: indent({ score: null, available: false }),
      mock: indent({ score: 712, available: true }),
    };
  }
  return {
    ...common,
    authType: 'none',
    header: '',
    secret: '',
    body: indent({}),
    outputKey: 'response',
    mock: indent({}),
  };
}

/** The form's values for a node's settings; missing settings take the preset's. */
export function connectorValues(config: Partial<ConnectorConfig> | undefined): ConnectorValues {
  const preset = config?.preset === 'bureau-score' ? 'bureau-score' : 'http';
  const defaults = presetValues(preset);
  if (!config || config.url === undefined) return { ...defaults, url: '' };
  const auth = config.auth ?? { type: 'none' };
  return {
    preset,
    url: config.url,
    authType: auth.type,
    header: auth.type === 'header' ? auth.header : '',
    secret: auth.type === 'none' ? '' : auth.secret,
    tokenUrl: auth.type === 'oauth2' ? auth.tokenUrl : '',
    clientId: auth.type === 'oauth2' ? auth.clientId : '',
    scopes: auth.type === 'oauth2' ? (auth.scopes ?? []).join(' ') : '',
    tls: config.tls !== undefined,
    certificate: config.tls?.certificate ?? '',
    key: config.tls?.key ?? '',
    ca: config.tls?.ca ?? '',
    body: indent(config.body ?? {}),
    outputKey: config.outputKey ?? defaults.outputKey,
    timeoutMs: config.timeoutMs === undefined ? '' : String(config.timeoutMs),
    retries: config.retries === undefined ? '' : String(config.retries),
    onError: config.onError ?? 'fail',
    fallback: config.fallback === undefined ? '' : indent(config.fallback),
    mock: config.mock === undefined ? '' : indent(config.mock),
  };
}

/** The settings the node stores, from valid form values. Empty optional fields are left out. */
export function connectorConfig(values: ConnectorValues): ConnectorConfig {
  const secret = values.secret.trim();
  const auth: Auth =
    values.authType === 'bearer'
      ? { type: 'bearer', secret }
      : values.authType === 'header'
        ? { type: 'header', header: values.header.trim(), secret }
        : values.authType === 'oauth2'
          ? {
              type: 'oauth2',
              tokenUrl: values.tokenUrl.trim(),
              clientId: values.clientId.trim(),
              secret,
              scopes: scopeWords(values.scopes),
            }
          : { type: 'none' };
  const ca = values.ca.trim();
  return {
    preset: values.preset,
    url: values.url.trim(),
    auth,
    ...(values.tls
      ? { tls: { certificate: values.certificate.trim(), key: values.key.trim(), ...(ca ? { ca } : {}) } }
      : {}),
    body: JSON.parse(values.body) as unknown,
    outputKey: values.outputKey.trim(),
    ...(values.timeoutMs.trim() ? { timeoutMs: Number(values.timeoutMs) } : {}),
    ...(values.retries.trim() ? { retries: Number(values.retries) } : {}),
    onError: values.onError,
    ...(values.onError === 'fallback' ? { fallback: JSON.parse(values.fallback) as unknown } : {}),
    mock: JSON.parse(values.mock) as unknown,
  };
}

/** `https://api.bureau.example/v2/score` → `api.bureau.example`; the text itself when it is no URL. */
export function urlHost(url: string): string {
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}
