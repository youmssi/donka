'use client';

import { useForm } from '@tanstack/react-form';
import { useTranslations } from 'next-intl';
import { useId, useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { SubmitButton } from '@/components/shared/form/submit-button';
import { TextField } from '@/components/shared/form/text-field';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Field, FieldDescription, FieldGroup, FieldLabel } from '@/components/ui/field';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';

import { createScenario, updateScenario } from './decision.service';
import { parseObject, SCENARIO_NAME_MAX, scenarioSchema, type Scenario, type ScenarioValues } from './schema';
import { useScenariosChanged } from './useDecisions';

/** JSON as the form shows it: indented, one field per line. */
export function jsonText(value: unknown): string {
  return JSON.stringify(value ?? {}, null, 2);
}

/** The form's values for an existing scenario. */
export function scenarioValues(scenario: Scenario): ScenarioValues {
  return {
    decisionId: scenario.decisionId,
    name: scenario.name,
    input: jsonText(scenario.input),
    expected: jsonText(scenario.expected),
    match: scenario.match,
  };
}

export interface ScenarioDialogProps {
  projectId: string;
  /** The decisions a new scenario may test; ignored when editing. */
  decisions: { id: string; key: string }[];
  /** Present: the dialog edits this scenario. */
  scenarioId?: string;
  /** What the form starts with (an existing scenario, or a simulator run). */
  initial: ScenarioValues;
  /** The decision is given (editing, or saving a simulator run). */
  fixedDecision?: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** Writes a test scenario: an input to a decision and the output it should give. */
export function ScenarioDialog({
  projectId,
  decisions,
  scenarioId,
  initial,
  fixedDecision,
  open,
  onOpenChange,
}: ScenarioDialogProps) {
  const t = useTranslations('scenarios');
  const common = useTranslations('common');
  const changed = useScenariosChanged(projectId);
  const [error, setError] = useState<ActionError | null>(null);
  const decisionField = useId();
  const matchField = useId();

  const form = useForm({
    defaultValues: initial,
    validators: { onChange: scenarioSchema, onSubmit: scenarioSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const values = {
        name: value.name.trim(),
        // Both parse: the schema checked them.
        input: parseObject(value.input) ?? {},
        expected: parseObject(value.expected) ?? {},
        match: value.match,
      };
      const result = scenarioId
        ? await updateScenario(projectId, scenarioId, values)
        : await createScenario(projectId, value.decisionId, values);
      if (result.ok) {
        changed();
        toast.success(scenarioId ? t('updated', { name: result.data.name }) : t('created', { name: result.data.name }));
        close();
      } else {
        setError(result.error);
      }
    },
  });

  function close() {
    onOpenChange(false);
    form.reset();
    setError(null);
  }

  return (
    <Dialog open={open} onOpenChange={(next) => (next ? onOpenChange(true) : close())}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{scenarioId ? t('editTitle') : t('createTitle')}</DialogTitle>
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
          {error && error.code !== 'SCENARIO_NAME_TAKEN' ? <ErrorAlert error={error} /> : null}
          <FieldGroup className="gap-2">
            <div className="grid gap-x-4 sm:grid-cols-2">
              <form.Field name="decisionId">
                {(field) => (
                  <Field className="gap-2 pb-7">
                    <FieldLabel htmlFor={decisionField}>{t('decision')}</FieldLabel>
                    <Select
                      value={field.state.value}
                      onValueChange={field.handleChange}
                      disabled={fixedDecision || Boolean(scenarioId)}
                    >
                      <SelectTrigger id={decisionField} className="w-full font-mono">
                        <SelectValue placeholder={t('chooseDecision')} />
                      </SelectTrigger>
                      <SelectContent>
                        {decisions.map((decision) => (
                          <SelectItem key={decision.id} value={decision.id} className="font-mono">
                            {decision.key}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </Field>
                )}
              </form.Field>
              <form.Field name="name">
                {(field) => (
                  <TextField
                    field={field}
                    label={t('name')}
                    placeholder={t('namePlaceholder')}
                    required
                    autoFocus
                    messageValues={{ max: SCENARIO_NAME_MAX }}
                    serverError={error?.code === 'SCENARIO_NAME_TAKEN' ? t('nameTaken') : undefined}
                  />
                )}
              </form.Field>
            </div>
            <div className="grid gap-x-4 sm:grid-cols-2">
              <form.Field name="input">
                {(field) => (
                  <TextField
                    field={field}
                    label={t('input')}
                    hint={t('inputHint')}
                    multiline
                    rows={8}
                    spellCheck={false}
                    className="font-mono text-xs"
                  />
                )}
              </form.Field>
              <form.Field name="expected">
                {(field) => (
                  <TextField
                    field={field}
                    label={t('expected')}
                    hint={t('expectedHint')}
                    multiline
                    rows={8}
                    spellCheck={false}
                    className="font-mono text-xs"
                  />
                )}
              </form.Field>
            </div>
            <form.Field name="match">
              {(field) => (
                <Field className="gap-2">
                  <FieldLabel htmlFor={matchField}>{t('match')}</FieldLabel>
                  <Select
                    value={field.state.value}
                    onValueChange={(value) => field.handleChange(value as typeof field.state.value)}
                  >
                    <SelectTrigger id={matchField} className="w-full sm:w-64">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="partial">{t('partial')}</SelectItem>
                      <SelectItem value="exact">{t('exact')}</SelectItem>
                    </SelectContent>
                  </Select>
                  <FieldDescription>
                    {field.state.value === 'partial' ? t('partialHint') : t('exactHint')}
                  </FieldDescription>
                </Field>
              )}
            </form.Field>
          </FieldGroup>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              {common('cancel')}
            </Button>
            <form.Subscribe selector={(state) => state.isSubmitting}>
              {(isSubmitting) => (
                <SubmitButton pending={isSubmitting} pendingLabel={t('saving')}>
                  {scenarioId ? t('save') : t('create')}
                </SubmitButton>
              )}
            </form.Subscribe>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
