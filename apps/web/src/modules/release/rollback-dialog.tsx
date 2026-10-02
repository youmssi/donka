'use client';

import { useForm } from '@tanstack/react-form';
import { Undo2 } from 'lucide-react';
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
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from '@/components/ui/field';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import type { Project } from '@/modules/project';

import { rollback } from './release.service';
import { REASON_MAX, rollbackSchema, type ReleaseSummary, type RollbackValues } from './schema';
import { useReleasesChanged, useRollbackTargets } from './useReleases';

/** Owners of an active project put an approved release back in production. */
export function canRollBack(project: Project): boolean {
  return project.role === 'owner' && !project.archivedAt;
}

/** "Roll back" when production has a release to go back to. */
export function RollbackButton({ project, live }: { project: Project; live: string | null }) {
  const t = useTranslations('rollback');
  const targets = useRollbackTargets(project.id, canRollBack(project)).data;
  const [open, setOpen] = useState(false);
  const items = targets?.ok ? targets.data : [];
  if (!canRollBack(project) || !live || items.length === 0) return null;
  return (
    <>
      <Button variant="outline" className="justify-self-start" onClick={() => setOpen(true)}>
        <Undo2 aria-hidden />
        {t('action')}
      </Button>
      {open ? <RollbackDialog project={project} targets={items} onClose={() => setOpen(false)} /> : null}
    </>
  );
}

function RollbackDialog({
  project,
  targets,
  onClose,
}: {
  project: Project;
  targets: ReleaseSummary[];
  onClose: () => void;
}) {
  const t = useTranslations('rollback');
  const common = useTranslations('common');
  const validation = useTranslations('validation');
  const changed = useReleasesChanged(project.id);
  const [error, setError] = useState<ActionError | null>(null);
  const releaseField = useId();

  const form = useForm({
    defaultValues: { releaseId: targets[0]?.id ?? '', reason: '' } as RollbackValues,
    validators: { onChange: rollbackSchema, onSubmit: rollbackSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await rollback(project.id, value.releaseId, value.reason.trim());
      changed();
      if (!result.ok) {
        setError(result.error);
        return;
      }
      onClose();
      toast.success(t('done', { version: result.data.releaseVersion }));
    },
  });

  return (
    <Dialog open onOpenChange={(open) => (open ? null : onClose())}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('title')}</DialogTitle>
          <DialogDescription>{t('description')}</DialogDescription>
        </DialogHeader>
        <form
          noValidate
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          {error ? <ErrorAlert error={error} /> : null}
          <FieldGroup className="gap-2">
            <form.Field name="releaseId">
              {(field) => (
                <Field className="gap-2 pb-7" data-invalid={field.state.meta.errors.length > 0 || undefined}>
                  <FieldLabel htmlFor={releaseField}>{t('release')}</FieldLabel>
                  <Select value={field.state.value} onValueChange={field.handleChange}>
                    <SelectTrigger id={releaseField} className="w-full overflow-hidden">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {targets.map((release) => (
                        <SelectItem key={release.id} value={release.id}>
                          <span className="font-mono">{release.version}</span>
                          <span className="max-w-60 min-w-0 truncate text-muted-foreground">{release.notes}</span>
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  {field.state.meta.errors.length > 0 ? (
                    <FieldError>{validation('required')}</FieldError>
                  ) : (
                    <FieldDescription>{t('releaseHint')}</FieldDescription>
                  )}
                </Field>
              )}
            </form.Field>
            <form.Field name="reason">
              {(field) => (
                <TextField
                  field={field}
                  label={t('reason')}
                  hint={t('reasonHint')}
                  multiline
                  rows={3}
                  required
                  autoFocus
                  messageValues={{ max: REASON_MAX }}
                />
              )}
            </form.Field>
          </FieldGroup>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              {common('cancel')}
            </Button>
            <form.Subscribe selector={(state) => [state.isSubmitting, state.values.releaseId] as const}>
              {([isSubmitting, releaseId]) => (
                <SubmitButton pending={isSubmitting} pendingLabel={t('rollingBack')}>
                  {t('submit', { version: targets.find((r) => r.id === releaseId)?.version ?? '' })}
                </SubmitButton>
              )}
            </form.Subscribe>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
