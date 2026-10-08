import {
  emptyField,
  fieldsToSchema,
  isValidPath,
  parseValues,
  readInputSchema,
  schemaToFields,
  writeInputSchema,
  type InputField,
} from './contract';

const age: InputField = {
  ...emptyField('applicant.age'),
  type: 'integer',
  minimum: 18,
  maximum: 99,
  label: { en: 'Age', fr: 'Âge' },
};
const nationalId: InputField = {
  ...emptyField('applicant.nationalId'),
  pii: true,
  help: { en: 'As on the ID card', fr: '' },
};
const employment: InputField = {
  ...emptyField('applicant.employment'),
  required: false,
  values: ['public', 'private'],
};
const amount: InputField = { ...emptyField('loan.amount'), type: 'number', required: false, minimum: 0 };

it('turns fields into a draft-07 schema with nested objects and hints', () => {
  expect(fieldsToSchema([age, nationalId, employment, amount])).toEqual({
    $schema: 'http://json-schema.org/draft-07/schema#',
    type: 'object',
    required: ['applicant'],
    properties: {
      applicant: {
        type: 'object',
        required: ['age', 'nationalId'],
        properties: {
          age: { type: 'integer', minimum: 18, maximum: 99, 'x-donka': { label: { en: 'Age', fr: 'Âge' } } },
          nationalId: { type: 'string', 'x-donka': { help: { en: 'As on the ID card' }, pii: true } },
          employment: { type: 'string', enum: ['public', 'private'] },
        },
      },
      loan: { type: 'object', properties: { amount: { type: 'number', minimum: 0 } } },
    },
  });
});

it('reads back the fields it wrote', () => {
  const fields = [age, nationalId, employment, amount];
  expect(schemaToFields(fieldsToSchema(fields))).toEqual(fields);
  expect(schemaToFields(null)).toEqual([]);
});

it('leaves a schema it cannot show faithfully to the JSON editor', () => {
  const arrays = { type: 'object', properties: { tags: { type: 'array', items: { type: 'string' } } } };
  const pattern = { type: 'object', properties: { iban: { type: 'string', pattern: '^CM' } } };
  const anyOf = { type: 'object', anyOf: [] };
  for (const schema of [arrays, pattern, anyOf]) expect(schemaToFields(schema)).toBeNull();
});

it('reads and writes the schema on the input node, as the editor stores it (a string)', () => {
  const graph = {
    nodes: [
      { id: 'in', type: 'inputNode', name: 'Request' },
      { id: 'out', type: 'outputNode', name: 'Response' },
    ],
    edges: [],
  };
  expect(readInputSchema(graph)).toBeNull();
  const schema = fieldsToSchema([age]);
  const written = writeInputSchema(graph, JSON.stringify(schema));
  expect(readInputSchema(written)).toEqual(schema);
  expect(readInputSchema(writeInputSchema(graph, '{ broken'))).toBeUndefined();
  expect(readInputSchema(writeInputSchema(graph, ''))).toBeNull();
  // The other nodes are untouched.
  expect((written as typeof graph).nodes[1]).toBe(graph.nodes[1]);
});

it('checks paths and allowed values', () => {
  expect(isValidPath('applicant.age')).toBe(true);
  for (const bad of ['', 'applicant.', '1st', 'monthly income', 'a..b']) expect(isValidPath(bad)).toBe(false);
  expect(parseValues(' public, private ,', 'string')).toEqual(['public', 'private']);
  expect(parseValues('12, 24, 36', 'integer')).toEqual([12, 24, 36]);
  expect(parseValues('12, 2.5', 'integer')).toBeNull();
  expect(parseValues('1.5, x', 'number')).toBeNull();
});
