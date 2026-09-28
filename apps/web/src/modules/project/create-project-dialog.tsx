'use client';

import { useForm } from '@tanstack/react-form';
import { Plus } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
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
import { useRouter } from '@/i18n/navigation';

import {
  createProjectSchema,
  DESCRIPTION_MAX,
  KEY_RULES,
  keyFromName,
  NAME_MAX,
  type CreateProjectValues,
} from './schema';
import { useCreateProject } from './useProjects';

export function CreateProjectDialog() {
  const t = useTranslations('projects');
  const common = useTranslations('common');
  const router = useRouter();
  const create = useCreateProject();
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<ActionError | null>(null);
  // The key follows the name until the person edits it themselves.
  const [keyEdited, setKeyEdited] = useState(false);

  const form = useForm({
    defaultValues: { name: '', key: '', description: '' } satisfies CreateProjectValues,
    validators: { onBlur: createProjectSchema, onSubmit: createProjectSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await create.mutateAsync(value);
      if (result.ok) {
        setOpen(false);
        router.push(`/projects/members?id=${result.data.id}`);
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
      setKeyEdited(false);
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
      <DialogContent closeLabel={common('close')}>
        <DialogHeader>
          <DialogTitle>{t('createTitle')}</DialogTitle>
          <DialogDescription>{t('createDescription')}</DialogDescription>
        </DialogHeader>
        <form
          noValidate
          className="grid gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          {error ? <ErrorAlert error={error} /> : null}
          <form.Field
            name="name"
            listeners={{
              onChange: ({ value }) => {
                if (!keyEdited) form.setFieldValue('key', keyFromName(value));
              },
            }}
          >
            {(field) => (
              <TextField field={field} label={t('name')} required autoFocus messageValues={{ max: NAME_MAX }} />
            )}
          </form.Field>
          <form.Field
            name="key"
            listeners={{
              onChange: () => setKeyEdited(true),
            }}
          >
            {(field) => (
              <TextField
                field={field}
                label={t('key')}
                hint={t('keyHint')}
                required
                spellCheck={false}
                autoCapitalize="off"
                className="font-mono"
                messageValues={{ min: KEY_RULES.min, max: KEY_RULES.max }}
              />
            )}
          </form.Field>
          <form.Field name="description">
            {(field) => (
              <TextField
                field={field}
                label={t('description')}
                hint={t('descriptionHint')}
                multiline
                messageValues={{ max: DESCRIPTION_MAX }}
              />
            )}
          </form.Field>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {common('cancel')}
            </Button>
            <form.Subscribe selector={(state) => state.isSubmitting}>
              {(isSubmitting) => (
                <Button type="submit" disabled={isSubmitting}>
                  {isSubmitting ? t('creating') : t('create')}
                </Button>
              )}
            </form.Subscribe>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
