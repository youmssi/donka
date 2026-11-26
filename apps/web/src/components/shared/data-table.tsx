'use client';

import { columnVisibilityFeature, tableFeatures, useTable, type ColumnDef, type RowData } from '@tanstack/react-table';
import { useTranslations } from 'next-intl';
import type { ReactNode } from 'react';

import { Pager } from '@/components/shared/pager';
import { Skeleton } from '@/components/ui/skeleton';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';

/**
 * Lists are paginated and ordered by the server, so the table only needs to know
 * which columns are shown; register another feature here when a list needs it.
 */
export const features = tableFeatures({ columnVisibilityFeature });
export type DataTableFeatures = typeof features;
export type DataTableColumn<TData extends RowData> = ColumnDef<DataTableFeatures, TData>;

interface Pagination {
  offset: number;
  pageSize: number;
  total: number;
  hrefFor: (offset: number) => string;
}

interface DataTableProps<TData extends RowData> {
  columns: DataTableColumn<TData>[];
  /** Absent while loading: the table shows skeleton rows. */
  data: TData[] | undefined;
  /** Shown instead of the table when there is no row. */
  empty: ReactNode;
  getRowId: (row: TData) => string;
  pagination?: Pagination;
  /** Accessible name of the table. */
  label: string;
}

const SKELETON_ROWS = 5;

/** The table every Studio list uses: shadcn Table rendered by TanStack Table. */
export function DataTable<TData extends RowData>({
  columns,
  data,
  empty,
  getRowId,
  pagination,
  label,
}: DataTableProps<TData>) {
  const common = useTranslations('common');
  const table = useTable({ features, columns, data: data ?? [], getRowId });

  if (data && data.length === 0) return <>{empty}</>;

  return (
    <div className="grid gap-3">
      <div className="overflow-hidden rounded-lg border">
        <Table aria-label={label} aria-busy={!data}>
          <TableHeader className="bg-muted/50">
            {table.getHeaderGroups().map((group) => (
              <TableRow key={group.id}>
                {group.headers.map((header) => (
                  <TableHead key={header.id} className={header.column.columnDef.meta?.className}>
                    {header.isPlaceholder ? null : <table.FlexRender header={header} />}
                  </TableHead>
                ))}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {data
              ? table.getRowModel().rows.map((row) => (
                  <TableRow key={row.id}>
                    {row.getVisibleCells().map((cell) => (
                      <TableCell key={cell.id} className={cell.column.columnDef.meta?.className}>
                        <table.FlexRender cell={cell} />
                      </TableCell>
                    ))}
                  </TableRow>
                ))
              : Array.from({ length: SKELETON_ROWS }, (_, index) => (
                  <TableRow key={index}>
                    {columns.map((_, column) => (
                      <TableCell key={column}>
                        <Skeleton className="h-4 w-full max-w-40" />
                      </TableCell>
                    ))}
                  </TableRow>
                ))}
          </TableBody>
        </Table>
        {data ? null : <span className="sr-only">{common('loading')}</span>}
      </div>
      {pagination && pagination.total > pagination.pageSize ? <Pager {...pagination} /> : null}
    </div>
  );
}
