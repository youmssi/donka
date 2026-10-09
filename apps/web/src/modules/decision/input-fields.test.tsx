import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';

import { renderWithProviders } from '@/test/render';

import { readInputSchema } from './contract';
import { InputFieldsSheet } from './input-fields';

const graph = {
  nodes: [
    { id: 'in', type: 'inputNode', name: 'Request', position: { x: 0, y: 0 } },
    { id: 'out', type: 'outputNode', name: 'Response', position: { x: 300, y: 0 } },
  ],
  edges: [],
};

/** The sheet over a graph kept in state, as the editor keeps the draft. */
function Harness({
  initial,
  editable = true,
  onChange,
}: {
  initial: unknown;
  editable?: boolean;
  onChange?: (g: unknown) => void;
}) {
  const [value, setValue] = useState(initial);
  const [open, setOpen] = useState(true);
  return (
    <InputFieldsSheet
      graph={value}
      onChange={(next) => {
        setValue(next);
        onChange?.(next);
      }}
      editable={editable}
      open={open}
      onOpenChange={setOpen}
    />
  );
}

it('adds a field in the table and writes the schema on the Request node', async () => {
  const user = userEvent.setup();
  const onChange = vi.fn();
  renderWithProviders(<Harness initial={graph} onChange={onChange} />);
  const sheet = await screen.findByRole('dialog', { name: 'Input fields' });
  expect(within(sheet).getByText('No input fields yet')).toBeInTheDocument();

  await user.click(within(sheet).getByRole('button', { name: 'Add field' }));
  const dialog = await screen.findByRole('dialog', { name: 'Add an input field' });
  await user.type(within(dialog).getByLabelText('Field'), 'monthly income');
  await user.click(within(dialog).getByRole('button', { name: 'Save field' }));
  expect(await within(dialog).findByText(/Use names of letters, digits and underscores/)).toBeInTheDocument();

  await user.clear(within(dialog).getByLabelText('Field'));
  await user.type(within(dialog).getByLabelText('Field'), 'applicant.employment');
  await user.type(within(dialog).getByLabelText('Allowed values'), 'public, private');
  await user.type(within(dialog).getByLabelText('Label (French)'), 'Emploi');
  await user.click(within(dialog).getByLabelText('Personal data (never sent to be explained)'));
  await user.click(within(dialog).getByRole('button', { name: 'Save field' }));

  expect(await within(sheet).findByText('applicant.employment')).toBeInTheDocument();
  expect(within(sheet).getByText('one of public, private')).toBeInTheDocument();
  expect(within(sheet).getByText('Personal data')).toBeInTheDocument();
  const schema = readInputSchema(onChange.mock.lastCall?.[0]);
  expect(schema).toMatchObject({
    type: 'object',
    required: ['applicant'],
    properties: {
      applicant: {
        required: ['employment'],
        properties: {
          employment: {
            type: 'string',
            enum: ['public', 'private'],
            'x-donka': { label: { fr: 'Emploi' }, pii: true },
          },
        },
      },
    },
  });
});

it('sends a schema the table cannot show to the JSON tab, and refuses invalid JSON there', async () => {
  const user = userEvent.setup();
  const withArray = {
    ...graph,
    nodes: [
      {
        ...graph.nodes[0],
        content: { schema: JSON.stringify({ type: 'object', properties: { tags: { type: 'array' } } }) },
      },
      graph.nodes[1],
    ],
  };
  const onChange = vi.fn();
  renderWithProviders(<Harness initial={withArray} onChange={onChange} />);
  const sheet = await screen.findByRole('dialog', { name: 'Input fields' });
  expect(within(sheet).getByText(/Edit it in the JSON Schema tab/)).toBeInTheDocument();
  expect(within(sheet).queryByRole('button', { name: 'Add field' })).not.toBeInTheDocument();

  await user.click(within(sheet).getByRole('tab', { name: 'JSON Schema' }));
  const text = within(sheet).getByLabelText('JSON Schema (draft-07)');
  await user.clear(text);
  await user.type(text, '{{ broken');
  await user.click(within(sheet).getByRole('button', { name: 'Apply' }));
  expect(within(sheet).getByText(/Enter a JSON object/)).toBeInTheDocument();
  expect(onChange).not.toHaveBeenCalled();
});

it('shows the fields without editing them to a viewer', async () => {
  const withField = {
    ...graph,
    nodes: [
      {
        ...graph.nodes[0],
        content: {
          schema: JSON.stringify({
            type: 'object',
            required: ['age'],
            properties: { age: { type: 'integer', minimum: 18 } },
          }),
        },
      },
      graph.nodes[1],
    ],
  };
  renderWithProviders(<Harness initial={withField} editable={false} />);
  const sheet = await screen.findByRole('dialog', { name: 'Input fields' });
  expect(within(sheet).getByText('age')).toBeInTheDocument();
  expect(within(sheet).getByText('at least 18')).toBeInTheDocument();
  expect(within(sheet).queryByRole('button', { name: 'Add field' })).not.toBeInTheDocument();
  expect(within(sheet).queryByRole('button', { name: 'Edit age' })).not.toBeInTheDocument();
});
