/**
 * The input contract of a decision (DNK-37): the JSON Schema on its input node, as the engine
 * reads it (draft-07, stored as a JSON string). The table edits flat fields (`applicant.age`);
 * these functions turn one into the other. Presentation hints live under `x-donka`.
 */

export const FIELD_TYPES = ['string', 'number', 'integer', 'boolean'] as const;
export type FieldType = (typeof FIELD_TYPES)[number];

export const STRING_FORMATS = ['', 'date', 'email'] as const;
export type StringFormat = (typeof STRING_FORMATS)[number];

export interface Texts {
  en: string;
  fr: string;
}

/** One request field as the table shows it. */
export interface InputField {
  /** Dotted path from the request's root (`applicant.age`). */
  path: string;
  type: FieldType;
  required: boolean;
  minimum?: number;
  maximum?: number;
  minLength?: number;
  maxLength?: number;
  /** Allowed values; empty means any. */
  values: (string | number)[];
  format: StringFormat;
  label: Texts;
  help: Texts;
  /** Personal data: never sent to be explained. */
  pii: boolean;
}

type Json = Record<string, unknown>;

const SCHEMA_ID = 'http://json-schema.org/draft-07/schema#';
const HINTS = 'x-donka';
/** Keywords the table edits; any other one means the schema is edited as JSON only. */
const LEAF_KEYS = new Set([
  'type',
  'minimum',
  'maximum',
  'minLength',
  'maxLength',
  'enum',
  'format',
  'title',
  'description',
  HINTS,
]);
const OBJECT_KEYS = new Set(['type', 'properties', 'required', '$schema', 'title', 'description', HINTS]);

function isObject(value: unknown): value is Json {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** A path segment: what a decision table or expression can read without quoting. */
export const SEGMENT = /^[A-Za-z_][A-Za-z0-9_]*$/;

export function isValidPath(path: string): boolean {
  return path.split('.').every((segment) => SEGMENT.test(segment));
}

export function emptyField(path = ''): InputField {
  return {
    path,
    type: 'string',
    required: true,
    values: [],
    format: '',
    label: { en: '', fr: '' },
    help: { en: '', fr: '' },
    pii: false,
  };
}

/** The input node of a graph, if any. */
function inputNode(graph: unknown): Json | undefined {
  if (!isObject(graph) || !Array.isArray(graph.nodes)) return undefined;
  return graph.nodes.find((node): node is Json => isObject(node) && node.type === 'inputNode');
}

/** The schema on the graph's input node: `null` when there is none, `undefined` when it does not parse. */
export function readInputSchema(graph: unknown): Json | null | undefined {
  const raw = inputNode(graph)?.content;
  const schema = isObject(raw) ? raw.schema : undefined;
  if (schema === undefined || schema === null || (typeof schema === 'string' && schema.trim() === '')) return null;
  if (isObject(schema)) return schema;
  if (typeof schema !== 'string') return undefined;
  try {
    const parsed: unknown = JSON.parse(schema);
    return isObject(parsed) ? parsed : undefined;
  } catch {
    return undefined;
  }
}

/** The graph with `schemaText` on its input node (the editor's storage: a JSON string). */
export function writeInputSchema(graph: unknown, schemaText: string): unknown {
  if (!isObject(graph) || !Array.isArray(graph.nodes)) return graph;
  return {
    ...graph,
    nodes: graph.nodes.map((node: unknown) =>
      isObject(node) && node.type === 'inputNode'
        ? { ...node, content: { ...(isObject(node.content) ? node.content : {}), schema: schemaText } }
        : node,
    ),
  };
}

export function hasInputNode(graph: unknown): boolean {
  return inputNode(graph) !== undefined;
}

function texts(value: unknown): Texts {
  return {
    en: isObject(value) && typeof value.en === 'string' ? value.en : '',
    fr: isObject(value) && typeof value.fr === 'string' ? value.fr : '',
  };
}

/**
 * The fields of a schema, in its order, or `null` when it uses something the table cannot
 * show faithfully (arrays, `anyOf`, a pattern…): it is then edited as JSON.
 */
export function schemaToFields(schema: Json | null): InputField[] | null {
  if (schema === null) return [];
  const fields: InputField[] = [];
  const walk = (node: Json, prefix: string, parentRequired: boolean): boolean => {
    if (Object.keys(node).some((key) => !OBJECT_KEYS.has(key))) return false;
    if (node.type !== 'object' || (node.properties !== undefined && !isObject(node.properties))) return false;
    const required = new Set(Array.isArray(node.required) ? node.required.filter((r) => typeof r === 'string') : []);
    for (const [name, property] of Object.entries(isObject(node.properties) ? node.properties : {})) {
      if (!isObject(property) || !SEGMENT.test(name)) return false;
      const path = prefix ? `${prefix}.${name}` : name;
      const isRequired = parentRequired && required.has(name);
      if (property.type === 'object') {
        if (!walk(property, path, isRequired)) return false;
        continue;
      }
      if (Object.keys(property).some((key) => !LEAF_KEYS.has(key))) return false;
      if (!FIELD_TYPES.includes(property.type as FieldType)) return false;
      const format = typeof property.format === 'string' ? property.format : '';
      if (!STRING_FORMATS.includes(format as StringFormat)) return false;
      const hints = isObject(property[HINTS]) ? property[HINTS] : {};
      const number = (key: string) => (typeof property[key] === 'number' ? (property[key] as number) : undefined);
      fields.push({
        path,
        type: property.type as FieldType,
        required: isRequired,
        minimum: number('minimum'),
        maximum: number('maximum'),
        minLength: number('minLength'),
        maxLength: number('maxLength'),
        values: Array.isArray(property.enum)
          ? property.enum.filter((v): v is string | number => typeof v === 'string' || typeof v === 'number')
          : [],
        format: format as StringFormat,
        label: texts(hints.label),
        help: texts(hints.help),
        pii: hints.pii === true,
      });
    }
    return true;
  };
  return walk(schema, '', true) ? fields : null;
}

/** The schema for `fields`: objects for dotted paths, `required` where every field below is. */
export function fieldsToSchema(fields: InputField[]): Json {
  const root: Json = { $schema: SCHEMA_ID, type: 'object', properties: {} };
  for (const field of fields) {
    const segments = field.path.split('.');
    let node = root;
    segments.forEach((segment, index) => {
      const properties = node.properties as Json;
      const last = index === segments.length - 1;
      const required = last
        ? field.required
        : fields.some((f) => f.path.startsWith(`${segments.slice(0, index + 1).join('.')}.`) && f.required);
      if (required) {
        const list = (node.required as string[] | undefined) ?? [];
        if (!list.includes(segment)) node.required = [...list, segment];
      }
      if (last) {
        properties[segment] = leaf(field);
      } else {
        properties[segment] ??= { type: 'object', properties: {} };
        node = properties[segment] as Json;
      }
    });
  }
  return root;
}

function leaf(field: InputField): Json {
  const schema: Json = { type: field.type };
  const numeric = field.type === 'number' || field.type === 'integer';
  if (numeric && field.minimum !== undefined) schema.minimum = field.minimum;
  if (numeric && field.maximum !== undefined) schema.maximum = field.maximum;
  if (field.type === 'string' && field.minLength !== undefined) schema.minLength = field.minLength;
  if (field.type === 'string' && field.maxLength !== undefined) schema.maxLength = field.maxLength;
  if (field.type === 'string' && field.format) schema.format = field.format;
  if (field.type !== 'boolean' && field.values.length > 0) schema.enum = field.values;
  const hints: Json = {};
  if (field.label.en || field.label.fr) hints.label = pick(field.label);
  if (field.help.en || field.help.fr) hints.help = pick(field.help);
  if (field.pii) hints.pii = true;
  if (Object.keys(hints).length > 0) schema[HINTS] = hints;
  return schema;
}

function pick(value: Texts): Partial<Texts> {
  const out: Partial<Texts> = {};
  if (value.en) out.en = value.en;
  if (value.fr) out.fr = value.fr;
  return out;
}

/** Allowed values typed as text: `public, private` or `12, 24, 36` for numbers. */
export function parseValues(text: string, type: FieldType): (string | number)[] | null {
  const parts = text
    .split(',')
    .map((part) => part.trim())
    .filter((part) => part !== '');
  if (type === 'number' || type === 'integer') {
    const numbers = parts.map(Number);
    const valid = numbers.every((n) => Number.isFinite(n) && (type === 'number' || Number.isInteger(n)));
    return valid ? numbers : null;
  }
  return parts;
}

/** The schema as the editor stores it: indented JSON, or nothing when there are no fields. */
export function schemaText(schema: Json | null): string {
  return schema === null ? '' : JSON.stringify(schema, null, 2);
}
