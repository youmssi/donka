'use client';

import { Plus } from 'lucide-react';
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
  DialogTrigger,
} from '@/components/ui/dialog';
import { FieldGroup } from '@/components/ui/field';
import { useRouter } from '@/i18n/navigation';

import { projectHome } from './links';
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

  const form = useAppForm({
    defaultValues: { name: '', key: '', description: '' } satisfies CreateProjectValues,
    validators: { onChange: createProjectSchema, onSubmit: createProjectSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      const result = await create.mutateAsync(value);
      if (result.ok) {
        setOpen(false);
        toast.success(t('created', { name: result.data.name }));
        router.push(projectHome(result.data.key));
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
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('createTitle')}</DialogTitle>
          <DialogDescription>{t('createDescription')}</DialogDescription>
        </DialogHeader>
        <form.AppForm>
          <form.Form className="grid gap-4">
            {error ? <ErrorAlert error={error} /> : null}
            <FieldGroup className="gap-2">
              <form.AppField
                name="name"
                listeners={{
                  onChange: ({ value }) => {
                    if (!keyEdited) form.setFieldValue('key', keyFromName(value), { dontRunListeners: true });
                  },
                }}
              >
                {(field) => <field.TextField label={t('name')} required autoFocus messageValues={{ max: NAME_MAX }} />}
              </form.AppField>
              <form.AppField
                name="key"
                listeners={{
                  onChange: () => setKeyEdited(true),
                }}
              >
                {(field) => (
                  <field.TextField
                    label={t('key')}
                    hint={t('keyHint')}
                    required
                    spellCheck={false}
                    autoCapitalize="off"
                    className="font-mono"
                    messageValues={{ min: KEY_RULES.min, max: KEY_RULES.max }}
                  />
                )}
              </form.AppField>
              <form.AppField name="description">
                {(field) => (
                  <field.TextField
                    label={t('description')}
                    hint={t('descriptionHint')}
                    multiline
                    messageValues={{ max: DESCRIPTION_MAX }}
                  />
                )}
              </form.AppField>
            </FieldGroup>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                {common('cancel')}
              </Button>
              <form.SubmitButton pendingLabel={t('creating')}>{t('create')}</form.SubmitButton>
            </DialogFooter>
          </form.Form>
        </form.AppForm>
      </DialogContent>
    </Dialog>
  );
}
