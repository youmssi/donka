'use client';

import { UserPlus } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { useAppForm } from '@/components/shared/form';
import { Alert, AlertDescription } from '@/components/ui/alert';
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
import { Spinner } from '@/components/ui/spinner';
import { useCurrentUser } from '@/modules/identity';
import { useInvite } from '@/modules/people';

import { addMemberSchema, ROLES, type AddMemberValues, type Project } from './schema';
import { useAddMember } from './useProjects';

const EMPTY: AddMemberValues = { email: '', role: 'viewer' };

/** Adds a person to the project; an administrator can invite someone who has no account yet. */
export function AddMemberDialog({ project }: { project: Project }) {
  const t = useTranslations('members');
  const roles = useTranslations('roles');
  const common = useTranslations('common');
  const me = useCurrentUser();
  const add = useAddMember(project.id);
  const invite = useInvite();
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<ActionError | null>(null);
  // What was last tried: an administrator can invite that unknown email and add it.
  const [attempt, setAttempt] = useState<AddMemberValues | null>(null);

  function close() {
    setOpen(false);
    setError(null);
    setAttempt(null);
    form.reset();
  }

  async function inviteAndAdd(values: AddMemberValues) {
    setError(null);
    const invited = await invite.mutateAsync({ email: values.email, locale: me.locale, isAdmin: false });
    if (!invited.ok) {
      setError(invited.error);
      return;
    }
    const result = await add.mutateAsync(values);
    if (result.ok) {
      toast.success(t('invitedAndAdded', { email: result.data.email }));
      close();
    } else {
      setError(result.error);
    }
  }

  const form = useAppForm({
    defaultValues: EMPTY,
    validators: { onChange: addMemberSchema, onSubmit: addMemberSchema },
    onSubmit: async ({ value }) => {
      setError(null);
      setAttempt(value);
      const result = await add.mutateAsync(value);
      if (result.ok) {
        toast.success(t('added', { email: result.data.email }));
        close();
      } else {
        setError(result.error);
      }
    },
  });

  return (
    <Dialog open={open} onOpenChange={(next) => (next ? setOpen(true) : close())}>
      <DialogTrigger asChild>
        <Button>
          <UserPlus aria-hidden />
          {t('addTitle')}
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('addTitle')}</DialogTitle>
          <DialogDescription>{t('addDescription')}</DialogDescription>
        </DialogHeader>
        <form.AppForm>
          <form.Form className="grid gap-4">
            {error?.code === 'NO_SUCH_USER' && me.isAdmin && attempt ? (
              // An administrator can fix this themselves: offer it instead of "ask an administrator".
              <Alert>
                <UserPlus aria-hidden />
                <AlertDescription>
                  <p>{t('noAccountAdmin', { email: attempt.email.trim() })}</p>
                  <Button
                    type="button"
                    size="sm"
                    className="mt-2"
                    disabled={invite.isPending || add.isPending}
                    onClick={() => void inviteAndAdd(attempt)}
                  >
                    {invite.isPending ? <Spinner /> : null}
                    {invite.isPending ? t('inviting') : t('inviteAndAdd', { email: attempt.email.trim() })}
                  </Button>
                </AlertDescription>
              </Alert>
            ) : error ? (
              <ErrorAlert error={error} />
            ) : null}
            <FieldGroup className="gap-2">
              <form.AppField name="email">
                {(field) => <field.TextField label={t('email')} type="email" autoComplete="off" required autoFocus />}
              </form.AppField>
              <form.AppField name="role">
                {(field) => (
                  <field.SelectField
                    label={t('role')}
                    hint={roles(`${field.state.value}Hint`)}
                    options={ROLES.map((role) => ({ value: role, label: roles(role) }))}
                  />
                )}
              </form.AppField>
            </FieldGroup>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={close}>
                {common('cancel')}
              </Button>
              <form.SubmitButton pendingLabel={t('adding')}>{t('add')}</form.SubmitButton>
            </DialogFooter>
          </form.Form>
        </form.AppForm>
      </DialogContent>
    </Dialog>
  );
}
