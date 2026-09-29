/* eslint-disable @typescript-eslint/no-unused-vars -- a declaration merge repeats the type parameters */
import type { CellData, RowData, TableFeatures } from '@tanstack/table-core';

declare module '@tanstack/table-core' {
  // Classes for a column's header and cells (width, alignment, hidden on phones).
  interface ColumnMeta<
    in out TFeatures extends TableFeatures,
    in out TData extends RowData,
    TValue extends CellData = CellData,
  > {
    className?: string;
  }
}
