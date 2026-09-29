'use client';

import { useForm } from '@tanstack/react-form';
import { Plus } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';

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
  DialogTrigger,
} from '@/components/ui/dialog';
import { FieldGroup } from '@/components/ui/field';
import { useRouter } from '@/i18n/navigation';
import { decisionHref, type Project } from '@/modules/project';

import { createDecisionSchema, KEY_RULES, type CreateDecisionValues } from './schema';
import { useCreateDecision } from './useDecisions';

/** Creates an empty decision and opens it in the editor. */
export function CreateDecisionDialog({ project }: { project: Project }) {
  const t = useTranslations('decisions');
  const common = useTranslations('common');
  const router = useRouter();
  const create = useCreateDecision(project.id);
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<ActionError | null>(null);

  const form = useForm({
    defaultValues: { key: '' } satisfies CreateDecisionValues,
    validators: { onChange: createDecisionSchema, onSubmit: createDecisionSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await create.mutateAsync({ key: value.key.trim() });
      if (result.ok) {
        setOpen(false);
        router.push(decisionHref(project.key, result.data.key));
      } else {
        setError(result.error);
      }
    },
  });

  function onOpenChange(next: boolean) {
    setOpen(next);
    if (!next) {
      form.reset();
      setError(null);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        <Button>
          <Plus aria-hidden />
          {t('create')}
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('createTitle')}</DialogTitle>
          <DialogDescription>{t('createDescription')}</DialogDescription>
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
            <form.Field name="key">
              {(field) => (
                <TextField
                  field={field}
                  label={t('key')}
                  hint={t('keyHint')}
                  placeholder="bureau/normalize"
                  required
                  autoFocus
                  spellCheck={false}
                  autoCapitalize="off"
                  className="font-mono"
                  messageValues={{ max: KEY_RULES.max }}
                  serverError={error?.code === 'DECISION_KEY_TAKEN' ? t('keyTaken') : undefined}
                />
              )}
            </form.Field>
          </FieldGroup>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {common('cancel')}
            </Button>
            <form.Subscribe selector={(state) => state.isSubmitting}>
              {(isSubmitting) => (
                <SubmitButton pending={isSubmitting} pendingLabel={t('creating')}>
                  {t('create')}
                </SubmitButton>
              )}
            </form.Subscribe>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
