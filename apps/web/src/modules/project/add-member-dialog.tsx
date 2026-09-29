'use client';

import { useForm } from '@tanstack/react-form';
import { UserPlus } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useId, useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { SubmitButton } from '@/components/shared/form/submit-button';
import { TextField } from '@/components/shared/form/text-field';
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
import { Field, FieldDescription, FieldGroup, FieldLabel } from '@/components/ui/field';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Spinner } from '@/components/ui/spinner';
import { useCurrentUser } from '@/modules/identity';
import { useInvite } from '@/modules/people';

import { addMemberSchema, ROLES, type AddMemberValues, type Project, type Role } from './schema';
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
  const roleId = useId();
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

  const form = useForm({
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
        <form
          noValidate
          className="grid gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
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
            <form.Field name="email">
              {(field) => (
                <TextField field={field} label={t('email')} type="email" autoComplete="off" required autoFocus />
              )}
            </form.Field>
            <form.Field name="role">
              {(field) => (
                <Field className="gap-2">
                  <FieldLabel htmlFor={roleId}>{t('role')}</FieldLabel>
                  <Select value={field.state.value} onValueChange={(role) => field.handleChange(role as Role)}>
                    <SelectTrigger id={roleId} className="w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {ROLES.map((role) => (
                        <SelectItem key={role} value={role}>
                          {roles(role)}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <FieldDescription>{roles(`${field.state.value}Hint`)}</FieldDescription>
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
                <SubmitButton pending={isSubmitting} pendingLabel={t('adding')}>
                  {t('add')}
                </SubmitButton>
              )}
            </form.Subscribe>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
