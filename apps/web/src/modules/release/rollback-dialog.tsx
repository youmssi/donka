'use client';

import { Undo2 } from 'lucide-react';
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
  const changed = useReleasesChanged(project.id);
  const [error, setError] = useState<ActionError | null>(null);

  const form = useAppForm({
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
        <form.AppForm>
          <form.Form className="grid gap-4">
            {error ? <ErrorAlert error={error} /> : null}
            <FieldGroup className="gap-2">
              <form.AppField name="releaseId">
                {(field) => (
                  <field.SelectField
                    label={t('release')}
                    hint={t('releaseHint')}
                    className="pb-7"
                    triggerClassName="overflow-hidden"
                    options={targets.map((release) => ({
                      value: release.id,
                      label: (
                        <>
                          <span className="font-mono">{release.version}</span>
                          <span className="max-w-60 min-w-0 truncate text-muted-foreground">{release.notes}</span>
                        </>
                      ),
                    }))}
                  />
                )}
              </form.AppField>
              <form.AppField name="reason">
                {(field) => (
                  <field.TextField
                    label={t('reason')}
                    hint={t('reasonHint')}
                    multiline
                    rows={3}
                    required
                    autoFocus
                    messageValues={{ max: REASON_MAX }}
                  />
                )}
              </form.AppField>
            </FieldGroup>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={onClose}>
                {common('cancel')}
              </Button>
              <form.Subscribe selector={(state) => state.values.releaseId}>
                {(releaseId) => (
                  <form.SubmitButton pendingLabel={t('rollingBack')}>
                    {t('submit', { version: targets.find((r) => r.id === releaseId)?.version ?? '' })}
                  </form.SubmitButton>
                )}
              </form.Subscribe>
            </DialogFooter>
          </form.Form>
        </form.AppForm>
      </DialogContent>
    </Dialog>
  );
}
