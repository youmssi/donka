import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { FolderOpen } from 'lucide-react';
import { useMemo } from 'react';

import { DataTable, type DataTableColumn } from '@/components/shared/data-table';
import { ErrorAlert } from '@/components/shared/error-alert';
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';

import { useStoryMessages, VIEW_STATES, type ViewState } from './fixtures';

interface Row {
  key: string;
  name: string;
}

function TablePreview({ state }: { state: ViewState }) {
  const m = useStoryMessages();
  const columns = useMemo<DataTableColumn<Row>[]>(
    () => [
      { accessorKey: 'name', header: m.projects.name, cell: (cell) => cell.row.original.name },
      {
        accessorKey: 'key',
        header: m.projects.key,
        cell: (cell) => <span className="font-mono">{cell.row.original.key}</span>,
      },
    ],
    [m],
  );
  const rows = useMemo(() => [{ key: 'order-discount', name: m.tokens.namePlaceholder }], [m]);
  if (state === 'error') return <ErrorAlert error={{ code: 'NETWORK' }} />;
  return (
    <DataTable
      columns={columns}
      data={state === 'loading' ? undefined : state === 'empty' ? [] : rows}
      getRowId={(row) => row.key}
      label={m.projects.title}
      empty={
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <FolderOpen aria-hidden />
            </EmptyMedia>
            <EmptyTitle>{m.projects.emptyTitle}</EmptyTitle>
            <EmptyDescription>{m.projects.emptyAdmin}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      }
      pagination={{ offset: 0, pageSize: 10, total: 30, hrefFor: (offset) => `?offset=${offset}` }}
    />
  );
}

const meta = {
  title: 'Shared/Data table',
  component: TablePreview,
  args: { state: 'success' },
  argTypes: { state: { control: 'select', options: VIEW_STATES } },
} satisfies Meta<typeof TablePreview>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Loading: Story = { args: { state: 'loading' } };
export const EmptyState: Story = { args: { state: 'empty' } };
export const ErrorState: Story = { args: { state: 'error' } };
export const Success: Story = {};
