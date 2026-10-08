'use client';

import { useForm } from '@tanstack/react-form';
import { ListChecks, Pencil, Plus, ShieldAlert, Trash2 } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useId, useMemo, useState } from 'react';
import { z } from 'zod';

import { TextField } from '@/components/shared/form/text-field';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { Field, FieldGroup, FieldLabel } from '@/components/ui/field';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle, SheetTrigger } from '@/components/ui/sheet';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Textarea } from '@/components/ui/textarea';

import {
  emptyField,
  FIELD_TYPES,
  fieldsToSchema,
  hasInputNode,
  isValidPath,
  parseValues,
  readInputSchema,
  schemaText,
  schemaToFields,
  STRING_FORMATS,
  writeInputSchema,
  type FieldType,
  type InputField,
  type StringFormat,
} from './contract';
import { parseObject } from './schema';

interface InputFieldsProps {
  graph: unknown;
  onChange: (graph: unknown) => void;
  editable: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * The decision's input contract (DNK-37): the fields a request must carry, edited as a table
 * or as JSON Schema. Both write the schema on the graph's input node, saved with the draft.
 */
export function InputFieldsSheet({ graph, onChange, editable, open, onOpenChange }: InputFieldsProps) {
  const t = useTranslations('inputFields');
  const schema = readInputSchema(graph);
  const fields = schema === undefined ? null : schemaToFields(schema);
  // The contract lives on the Request node: without one there is nowhere to keep it.
  const canEdit = editable && hasInputNode(graph);
  const [editing, setEditing] = useState<{ index: number | null; field: InputField } | null>(null);

  function writeFields(next: InputField[]) {
    onChange(writeInputSchema(graph, next.length === 0 ? '' : schemaText(fieldsToSchema(next))));
  }

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetTrigger asChild>
        <Button variant="outline" size="sm">
          <ListChecks aria-hidden />
          {t('open')}
          {fields && fields.length > 0 ? <Badge variant="secondary">{fields.length}</Badge> : null}
        </Button>
      </SheetTrigger>
      <SheetContent className="w-full gap-0 sm:max-w-4xl">
        <SheetHeader>
          <SheetTitle>{t('title')}</SheetTitle>
          <SheetDescription>{t('description')}</SheetDescription>
        </SheetHeader>
        <Tabs defaultValue="fields" className="min-h-0 flex-1 gap-3 overflow-y-auto px-4 pb-4">
          <TabsList>
            <TabsTrigger value="fields">{t('fieldsTab')}</TabsTrigger>
            <TabsTrigger value="json">{t('jsonTab')}</TabsTrigger>
          </TabsList>
          <TabsContent value="fields" className="grid gap-3">
            {editable && !hasInputNode(graph) ? (
              <Alert>
                <ShieldAlert aria-hidden />
                <AlertDescription>{t('noInputNode')}</AlertDescription>
              </Alert>
            ) : null}
            {fields === null ? (
              <Alert>
                <ShieldAlert aria-hidden />
                <AlertDescription>{schema === undefined ? t('unreadable') : t('jsonOnly')}</AlertDescription>
              </Alert>
            ) : (
              <FieldsTable
                fields={fields}
                editable={canEdit}
                onEdit={(index) => setEditing({ index, field: fields[index] ?? emptyField() })}
                onRemove={(index) => writeFields(fields.filter((_, i) => i !== index))}
              />
            )}
            {canEdit && fields !== null ? (
              <div>
                <Button size="sm" onClick={() => setEditing({ index: null, field: emptyField() })}>
                  <Plus aria-hidden />
                  {t('add')}
                </Button>
              </div>
            ) : null}
          </TabsContent>
          <TabsContent value="json">
            <JsonEditor
              // A new text when the table changes the schema.
              key={JSON.stringify(schema ?? null)}
              initial={schema === undefined ? currentText(graph) : schemaText(schema)}
              editable={canEdit}
              onApply={(text) => onChange(writeInputSchema(graph, text))}
            />
          </TabsContent>
        </Tabs>
        {editing && fields ? (
          <FieldDialog
            initial={editing.field}
            taken={fields.filter((_, i) => i !== editing.index).map((f) => f.path)}
            onCancel={() => setEditing(null)}
            onSave={(field) => {
              const next =
                editing.index === null ? [...fields, field] : fields.map((f, i) => (i === editing.index ? field : f));
              writeFields(next);
              setEditing(null);
            }}
          />
        ) : null}
      </SheetContent>
    </Sheet>
  );
}

/** The schema text as stored, for a schema that does not parse. */
function currentText(graph: unknown): string {
  if (typeof graph !== 'object' || graph === null || !('nodes' in graph) || !Array.isArray(graph.nodes)) return '';
  const input = (graph.nodes as { type?: string; content?: { schema?: unknown } }[]).find(
    (n) => n.type === 'inputNode',
  );
  return typeof input?.content?.schema === 'string' ? input.content.schema : '';
}

function FieldsTable({
  fields,
  editable,
  onEdit,
  onRemove,
}: {
  fields: InputField[];
  editable: boolean;
  onEdit: (index: number) => void;
  onRemove: (index: number) => void;
}) {
  const t = useTranslations('inputFields');
  if (fields.length === 0) {
    return (
      <Empty className="border border-dashed">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <ListChecks />
          </EmptyMedia>
          <EmptyTitle>{t('emptyTitle')}</EmptyTitle>
          <EmptyDescription>{t('emptyDescription')}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }
  return (
    <div className="overflow-x-auto rounded-md border">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>{t('field')}</TableHead>
            <TableHead className="hidden sm:table-cell">{t('type')}</TableHead>
            <TableHead>{t('rules')}</TableHead>
            <TableHead className="hidden sm:table-cell">{t('label')}</TableHead>
            {editable ? <TableHead className="w-0" /> : null}
          </TableRow>
        </TableHeader>
        <TableBody>
          {fields.map((field, index) => (
            <TableRow key={field.path}>
              <TableCell className="font-mono text-xs [overflow-wrap:anywhere] whitespace-normal sm:[overflow-wrap:normal] sm:whitespace-nowrap">
                {field.path}
                <span className="block font-sans text-sm sm:hidden">{t(`types.${field.type}`)}</span>
                <div className="mt-1 flex flex-wrap gap-1 font-sans">
                  {field.required ? <Badge variant="outline">{t('required')}</Badge> : null}
                  {field.pii ? <Badge variant="secondary">{t('pii')}</Badge> : null}
                </div>
              </TableCell>
              <TableCell className="hidden sm:table-cell">{t(`types.${field.type}`)}</TableCell>
              <TableCell className="min-w-28 text-sm whitespace-normal text-muted-foreground sm:min-w-40">
                <RulesSummary field={field} />
              </TableCell>
              <TableCell className="hidden min-w-36 text-sm whitespace-normal sm:table-cell">
                {field.label.en || field.label.fr ? (
                  <>
                    <span>{field.label.en}</span>
                    {field.label.fr ? <span className="block text-muted-foreground">{field.label.fr}</span> : null}
                  </>
                ) : (
                  <span className="text-muted-foreground">—</span>
                )}
              </TableCell>
              {editable ? (
                <TableCell>
                  <div className="flex flex-col gap-1 sm:flex-row">
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => onEdit(index)}
                      aria-label={t('edit', { path: field.path })}
                    >
                      <Pencil aria-hidden />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => onRemove(index)}
                      aria-label={t('remove', { path: field.path })}
                    >
                      <Trash2 aria-hidden />
                    </Button>
                  </div>
                </TableCell>
              ) : null}
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

/** Limits, allowed values and format in a few words. */
function RulesSummary({ field }: { field: InputField }) {
  const t = useTranslations('inputFields');
  const parts: string[] = [];
  const range = (min?: number, max?: number) =>
    min !== undefined && max !== undefined
      ? t('between', { min, max })
      : min !== undefined
        ? t('atLeast', { min })
        : max !== undefined
          ? t('atMost', { max })
          : null;
  const numbers = range(field.minimum, field.maximum);
  const lengths = range(field.minLength, field.maxLength);
  if (numbers) parts.push(numbers);
  if (lengths) parts.push(t('characters', { range: lengths }));
  if (field.values.length > 0) parts.push(t('oneOf', { values: field.values.join(', ') }));
  if (field.format) parts.push(t(`formats.${field.format}`));
  return parts.length > 0 ? parts.join(' · ') : <span>—</span>;
}

const optionalNumber = z.string().refine((text) => text.trim() === '' || Number.isFinite(Number(text)), 'number');

function fieldFormSchema(taken: string[]) {
  return z
    .object({
      path: z
        .string()
        .trim()
        .min(1, 'required')
        .refine(isValidPath, 'fieldPathSegments')
        .refine((path) => !taken.includes(path), 'fieldPathTaken'),
      type: z.enum(FIELD_TYPES),
      required: z.boolean(),
      minimum: optionalNumber,
      maximum: optionalNumber,
      minLength: optionalNumber,
      maxLength: optionalNumber,
      values: z.string(),
      format: z.enum(STRING_FORMATS),
      labelEn: z.string(),
      labelFr: z.string(),
      helpEn: z.string(),
      helpFr: z.string(),
      pii: z.boolean(),
    })
    .superRefine((value, ctx) => {
      if (parseValues(value.values, value.type) === null) {
        ctx.addIssue({ code: 'custom', path: ['values'], message: 'allowedValues' });
      }
      for (const [min, max] of [
        ['minimum', 'maximum'],
        ['minLength', 'maxLength'],
      ] as const) {
        const low = toNumber(value[min]);
        const high = toNumber(value[max]);
        if (low !== undefined && high !== undefined && low > high) {
          ctx.addIssue({ code: 'custom', path: [max], message: 'minAboveMax' });
        }
      }
    });
}

type FieldValues = z.infer<ReturnType<typeof fieldFormSchema>>;

function toNumber(text: string): number | undefined {
  return text.trim() === '' ? undefined : Number(text);
}

function toValues(field: InputField): FieldValues {
  const text = (n?: number) => (n === undefined ? '' : String(n));
  return {
    path: field.path,
    type: field.type,
    required: field.required,
    minimum: text(field.minimum),
    maximum: text(field.maximum),
    minLength: text(field.minLength),
    maxLength: text(field.maxLength),
    values: field.values.join(', '),
    format: field.format,
    labelEn: field.label.en,
    labelFr: field.label.fr,
    helpEn: field.help.en,
    helpFr: field.help.fr,
    pii: field.pii,
  };
}

function toField(value: FieldValues): InputField {
  const numeric = value.type === 'number' || value.type === 'integer';
  const text = value.type === 'string';
  return {
    path: value.path.trim(),
    type: value.type,
    required: value.required,
    minimum: numeric ? toNumber(value.minimum) : undefined,
    maximum: numeric ? toNumber(value.maximum) : undefined,
    minLength: text ? toNumber(value.minLength) : undefined,
    maxLength: text ? toNumber(value.maxLength) : undefined,
    values: value.type === 'boolean' ? [] : (parseValues(value.values, value.type) ?? []),
    format: text ? value.format : '',
    label: { en: value.labelEn.trim(), fr: value.labelFr.trim() },
    help: { en: value.helpEn.trim(), fr: value.helpFr.trim() },
    pii: value.pii,
  };
}

function FieldDialog({
  initial,
  taken,
  onCancel,
  onSave,
}: {
  initial: InputField;
  taken: string[];
  onCancel: () => void;
  onSave: (field: InputField) => void;
}) {
  const t = useTranslations('inputFields');
  const common = useTranslations('common');
  const typeId = useId();
  const formatId = useId();
  const schema = useMemo(() => fieldFormSchema(taken), [taken]);
  const form = useForm({
    defaultValues: toValues(initial),
    validators: { onChange: schema, onSubmit: schema },
    onSubmit: ({ value }) => onSave(toField(value)),
  });

  return (
    <Dialog open onOpenChange={(open) => (open ? null : onCancel())}>
      <DialogContent className="max-h-[90dvh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{initial.path ? t('editTitle', { path: initial.path }) : t('addTitle')}</DialogTitle>
          <DialogDescription>{t('dialogDescription')}</DialogDescription>
        </DialogHeader>
        <form
          noValidate
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          <FieldGroup className="gap-4">
            <div className="grid gap-4 sm:grid-cols-[2fr_1fr]">
              <form.Field name="path">
                {(field) => (
                  <TextField
                    field={field}
                    label={t('field')}
                    hint={t('pathHint')}
                    placeholder="applicant.age"
                    className="font-mono"
                    autoFocus
                    required
                  />
                )}
              </form.Field>
              <form.Field name="type">
                {(field) => (
                  <Field className="gap-2">
                    <FieldLabel htmlFor={typeId}>{t('type')}</FieldLabel>
                    <Select value={field.state.value} onValueChange={(v) => field.handleChange(v as FieldType)}>
                      <SelectTrigger id={typeId} className="w-full">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {FIELD_TYPES.map((type) => (
                          <SelectItem key={type} value={type}>
                            {t(`types.${type}`)}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </Field>
                )}
              </form.Field>
            </div>
            <div className="flex flex-wrap gap-6">
              <form.Field name="required">
                {(field) => (
                  <CheckboxField label={t('required')} checked={field.state.value} onChange={field.handleChange} />
                )}
              </form.Field>
              <form.Field name="pii">
                {(field) => (
                  <CheckboxField label={t('piiLong')} checked={field.state.value} onChange={field.handleChange} />
                )}
              </form.Field>
            </div>
            <form.Subscribe selector={(state) => state.values.type}>
              {(type) => (
                <>
                  {type === 'number' || type === 'integer' ? (
                    <div className="grid gap-4 sm:grid-cols-2">
                      <form.Field name="minimum">
                        {(field) => <TextField field={field} label={t('minimum')} inputMode="decimal" />}
                      </form.Field>
                      <form.Field name="maximum">
                        {(field) => <TextField field={field} label={t('maximum')} inputMode="decimal" />}
                      </form.Field>
                    </div>
                  ) : null}
                  {type === 'string' ? (
                    <div className="grid gap-4 sm:grid-cols-3">
                      <form.Field name="minLength">
                        {(field) => <TextField field={field} label={t('minLength')} inputMode="numeric" />}
                      </form.Field>
                      <form.Field name="maxLength">
                        {(field) => <TextField field={field} label={t('maxLength')} inputMode="numeric" />}
                      </form.Field>
                      <form.Field name="format">
                        {(field) => (
                          <Field className="gap-2">
                            <FieldLabel htmlFor={formatId}>{t('format')}</FieldLabel>
                            <Select
                              value={field.state.value || 'none'}
                              onValueChange={(v) => field.handleChange((v === 'none' ? '' : v) as StringFormat)}
                            >
                              <SelectTrigger id={formatId} className="w-full">
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent>
                                <SelectItem value="none">{t('formats.none')}</SelectItem>
                                <SelectItem value="date">{t('formats.date')}</SelectItem>
                                <SelectItem value="email">{t('formats.email')}</SelectItem>
                              </SelectContent>
                            </Select>
                          </Field>
                        )}
                      </form.Field>
                    </div>
                  ) : null}
                  {type !== 'boolean' ? (
                    <form.Field name="values">
                      {(field) => (
                        <TextField
                          field={field}
                          label={t('values')}
                          hint={t('valuesHint')}
                          placeholder={type === 'string' ? 'public, private' : '12, 24, 36'}
                        />
                      )}
                    </form.Field>
                  ) : null}
                </>
              )}
            </form.Subscribe>
            <div className="grid gap-4 sm:grid-cols-2">
              <form.Field name="labelEn">{(field) => <TextField field={field} label={t('labelEn')} />}</form.Field>
              <form.Field name="labelFr">{(field) => <TextField field={field} label={t('labelFr')} />}</form.Field>
              <form.Field name="helpEn">{(field) => <TextField field={field} label={t('helpEn')} />}</form.Field>
              <form.Field name="helpFr">{(field) => <TextField field={field} label={t('helpFr')} />}</form.Field>
            </div>
          </FieldGroup>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onCancel}>
              {common('cancel')}
            </Button>
            <Button type="submit">{t('saveField')}</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function CheckboxField({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  const id = useId();
  return (
    <Field orientation="horizontal" className="w-auto gap-2">
      <Checkbox id={id} checked={checked} onCheckedChange={(value) => onChange(value === true)} />
      <FieldLabel htmlFor={id} className="font-normal">
        {label}
      </FieldLabel>
    </Field>
  );
}

/** The schema as JSON, for what the table cannot express (arrays, patterns, `anyOf`…). */
function JsonEditor({
  initial,
  editable,
  onApply,
}: {
  initial: string;
  editable: boolean;
  onApply: (text: string) => void;
}) {
  const t = useTranslations('inputFields');
  const validation = useTranslations('validation');
  const id = useId();
  const [text, setText] = useState(initial);
  const [invalid, setInvalid] = useState(false);
  return (
    <div className="grid gap-2">
      <FieldLabel htmlFor={id}>{t('jsonLabel')}</FieldLabel>
      <Textarea
        id={id}
        value={text}
        onChange={(event) => {
          setText(event.target.value);
          setInvalid(false);
        }}
        readOnly={!editable}
        rows={18}
        spellCheck={false}
        className="font-mono text-xs"
        aria-invalid={invalid || undefined}
        aria-describedby={`${id}-hint`}
      />
      <p id={`${id}-hint`} className="text-sm text-muted-foreground">
        {invalid ? <span className="text-destructive">{validation('jsonObject')}</span> : t('jsonHint')}
      </p>
      {editable ? (
        <div>
          <Button
            size="sm"
            disabled={text === initial}
            onClick={() => {
              if (text.trim() !== '' && parseObject(text) === null) {
                setInvalid(true);
                return;
              }
              onApply(text.trim());
            }}
          >
            {t('applyJson')}
          </Button>
        </div>
      ) : null}
    </div>
  );
}
