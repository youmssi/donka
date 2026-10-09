'use client';

import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { FieldGroup } from '@/components/ui/field';

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

  const form = useAppForm({
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
        <form.AppForm>
          <form.Form className="grid gap-4">
            {error && error.code !== 'SCENARIO_NAME_TAKEN' ? <ErrorAlert error={error} /> : null}
            <FieldGroup className="gap-2">
              <div className="grid gap-x-4 sm:grid-cols-2">
                <form.AppField name="decisionId">
                  {(field) => (
                    <field.SelectField
                      label={t('decision')}
                      placeholder={t('chooseDecision')}
                      className="pb-7"
                      triggerClassName="font-mono"
                      disabled={fixedDecision || Boolean(scenarioId)}
                      options={decisions.map((decision) => ({
                        value: decision.id,
                        label: decision.key,
                        className: 'font-mono',
                      }))}
                    />
                  )}
                </form.AppField>
                <form.AppField name="name">
                  {(field) => (
                    <field.TextField
                      label={t('name')}
                      placeholder={t('namePlaceholder')}
                      required
                      autoFocus
                      messageValues={{ max: SCENARIO_NAME_MAX }}
                      serverError={error?.code === 'SCENARIO_NAME_TAKEN' ? t('nameTaken') : undefined}
                    />
                  )}
                </form.AppField>
              </div>
              <div className="grid gap-x-4 sm:grid-cols-2">
                <form.AppField name="input">
                  {(field) => (
                    <field.TextField
                      label={t('input')}
                      hint={t('inputHint')}
                      multiline
                      rows={8}
                      spellCheck={false}
                      className="font-mono text-xs"
                    />
                  )}
                </form.AppField>
                <form.AppField name="expected">
                  {(field) => (
                    <field.TextField
                      label={t('expected')}
                      hint={t('expectedHint')}
                      multiline
                      rows={8}
                      spellCheck={false}
                      className="font-mono text-xs"
                    />
                  )}
                </form.AppField>
              </div>
              <form.AppField name="match">
                {(field) => (
                  <field.SelectField
                    label={t('match')}
                    hint={field.state.value === 'partial' ? t('partialHint') : t('exactHint')}
                    triggerClassName="sm:w-64"
                    options={[
                      { value: 'partial', label: t('partial') },
                      { value: 'exact', label: t('exact') },
                    ]}
                  />
                )}
              </form.AppField>
            </FieldGroup>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={close}>
                {common('cancel')}
              </Button>
              <form.SubmitButton pendingLabel={t('saving')}>{scenarioId ? t('save') : t('create')}</form.SubmitButton>
            </DialogFooter>
          </form.Form>
        </form.AppForm>
      </DialogContent>
    </Dialog>
  );
}
