'use client';

import { useForm } from '@tanstack/react-form';
import { CircleAlert, Plus, TriangleAlert } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useId, useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { SubmitButton } from '@/components/shared/form/submit-button';
import { TextField } from '@/components/shared/form/text-field';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
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
import { Skeleton } from '@/components/ui/skeleton';
import { Link } from '@/i18n/navigation';
import { decisionHref, type Project } from '@/modules/project';

import { FrozenDecisions } from './frozen-decisions';
import { createRelease, deployRelease } from './release.service';
import { NOTES_MAX, releaseSchema, type Bump, type ReleasePreview, type ReleaseValues } from './schema';
import { useReleasePreview, useReleasesChanged } from './useReleases';

/** "New release": what would be frozen, the version, the notes. */
export function NewRelease({ project }: { project: Project }) {
  const t = useTranslations('releases');
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button onClick={() => setOpen(true)}>
        <Plus aria-hidden />
        {t('create')}
      </Button>
      {open ? <ReleaseDialog project={project} onClose={() => setOpen(false)} /> : null}
    </>
  );
}

function ReleaseDialog({ project, onClose }: { project: Project; onClose: () => void }) {
  const t = useTranslations('releases');
  const common = useTranslations('common');
  const preview = useReleasePreview(project.id, true);
  const result = preview.data;

  return (
    <Dialog open onOpenChange={(open) => (open ? null : onClose())}>
      <DialogContent className="max-h-[90dvh] grid-rows-[auto_1fr] sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{t('createTitle')}</DialogTitle>
          <DialogDescription>{t('createDescription')}</DialogDescription>
        </DialogHeader>
        {!result ? (
          <div className="grid gap-3">
            <Skeleton className="h-9 w-full" />
            <Skeleton className="h-24 w-full" />
          </div>
        ) : !result.ok ? (
          <ErrorAlert
            error={result.error}
            action={
              <Button variant="outline" size="sm" className="mt-2" onClick={() => void preview.refetch()}>
                {common('retry')}
              </Button>
            }
          />
        ) : result.data.unversioned.length > 0 ? (
          <Unversioned project={project} keys={result.data.unversioned} onClose={onClose} />
        ) : result.data.decisions.length === 0 ? (
          <Alert>
            <CircleAlert aria-hidden />
            <AlertTitle>{t('nothingTitle')}</AlertTitle>
            <AlertDescription>{t('nothing')}</AlertDescription>
          </Alert>
        ) : (
          <ReleaseForm project={project} preview={result.data} onClose={onClose} />
        )}
      </DialogContent>
    </Dialog>
  );
}

/** Every decision must have a version: say which do not, with a way to fix it. */
function Unversioned({ project, keys, onClose }: { project: Project; keys: string[]; onClose: () => void }) {
  const t = useTranslations('releases');
  const common = useTranslations('common');
  return (
    <div className="grid gap-4">
      <Alert variant="destructive">
        <CircleAlert aria-hidden />
        <AlertTitle>{t('unversionedTitle')}</AlertTitle>
        <AlertDescription>
          <p>{t('unversioned')}</p>
          <ul className="mt-1 grid gap-1">
            {keys.map((key) => (
              <li key={key}>
                <Link href={decisionHref(project.key, key)} className="font-mono underline underline-offset-4">
                  {key}
                </Link>
              </li>
            ))}
          </ul>
        </AlertDescription>
      </Alert>
      <DialogFooter>
        <Button variant="outline" onClick={onClose}>
          {common('close')}
        </Button>
      </DialogFooter>
    </div>
  );
}

function ReleaseForm({
  project,
  preview,
  onClose,
}: {
  project: Project;
  preview: ReleasePreview;
  onClose: () => void;
}) {
  const t = useTranslations('releases');
  const common = useTranslations('common');
  const errors = useTranslations('errors');
  const changed = useReleasesChanged(project.id);
  const [error, setError] = useState<ActionError | null>(null);
  const bumpField = useId();
  const failing = preview.decisions.some((d) => d.tests.failed + d.tests.errors > 0);

  const form = useForm({
    defaultValues: { bump: 'minor', notes: '' } as ReleaseValues,
    validators: { onChange: releaseSchema, onSubmit: releaseSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await createRelease(project.id, { bump: value.bump, notes: value.notes.trim() });
      if (!result.ok) {
        setError(result.error);
        return;
      }
      const release = result.data;
      changed();
      onClose();
      toast.success(t('created', { version: release.version }), {
        action: {
          label: t('deployStaging'),
          onClick: () =>
            void deployRelease(project.id, 'staging', release.id).then((deployed) => {
              changed();
              if (deployed.ok) toast.success(t('deploying', { version: release.version }));
              else toast.error(errors(deployed.error.code));
            }),
        },
      });
    },
  });

  return (
    <form
      noValidate
      className="grid min-h-0 gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
    >
      {error ? <ErrorAlert error={error} /> : null}
      <div className="-mx-6 grid min-h-0 gap-4 overflow-y-auto px-6">
        <FieldGroup className="gap-2">
          {preview.latest ? (
            <form.Field name="bump">
              {(field) => (
                <Field className="gap-2">
                  <FieldLabel htmlFor={bumpField}>{t('version')}</FieldLabel>
                  <Select value={field.state.value} onValueChange={(value) => field.handleChange(value as Bump)}>
                    <SelectTrigger id={bumpField} className="w-full sm:w-72">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {(['major', 'minor', 'patch'] as const).map((bump) => (
                        <SelectItem key={bump} value={bump}>
                          <span className="font-mono">{preview.next[bump]}</span>
                          <span className="text-muted-foreground">· {t(bump)}</span>
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <FieldDescription>{t('latest', { version: preview.latest ?? '' })}</FieldDescription>
                </Field>
              )}
            </form.Field>
          ) : (
            <Field className="gap-1">
              <FieldLabel>{t('version')}</FieldLabel>
              <p className="font-mono text-lg font-semibold">{preview.next.minor}</p>
              <FieldDescription>{t('firstRelease')}</FieldDescription>
            </Field>
          )}
          <form.Field name="notes">
            {(field) => (
              <TextField
                field={field}
                label={t('notes')}
                hint={t('notesHint')}
                multiline
                rows={4}
                required
                autoFocus
                messageValues={{ max: NOTES_MAX }}
              />
            )}
          </form.Field>
        </FieldGroup>
        {failing ? (
          <Alert>
            <TriangleAlert aria-hidden />
            <AlertTitle>{t('failingTitle')}</AlertTitle>
            <AlertDescription>{t('failing')}</AlertDescription>
          </Alert>
        ) : null}
        <FrozenDecisions decisions={preview.decisions} />
      </div>
      <DialogFooter>
        <Button type="button" variant="outline" onClick={onClose}>
          {common('cancel')}
        </Button>
        <form.Subscribe selector={(state) => [state.isSubmitting, state.values.bump] as const}>
          {([isSubmitting, bump]) => (
            <SubmitButton pending={isSubmitting} pendingLabel={t('creating')}>
              {t('createVersion', { version: preview.next[bump] })}
            </SubmitButton>
          )}
        </form.Subscribe>
      </DialogFooter>
    </form>
  );
}
