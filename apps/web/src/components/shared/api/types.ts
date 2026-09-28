import type { components } from './schema';

/** Schemas of the Studio API, generated from apps/web/openapi.json (`pnpm api:generate`). */
export type ApiSchemas = components['schemas'];
export type ErrorBody = ApiSchemas['ErrorBody'];
