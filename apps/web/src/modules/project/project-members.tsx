'use client';

import { useForm } from '@tanstack/react-form';
import { CircleCheck, UserPlus } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useId, useState } from 'react';

import type { ActionError } from '@/components/shared/api';
import { ErrorAlert } from '@/components/shared/error-alert';
import { TextField } from '@/components/shared/form/text-field';
import { PageSkeleton } from '@/components/shared/page-skeleton';
import { Alert, AlertDescription } from '@/components/ui/alert';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { useCurrentUser } from '@/modules/identity';
import { useInvite } from '@/modules/people';

import { ProjectFrame } from './project-frame';
import { RoleBadge } from './role-badge';
import { addMemberSchema, ROLES, type AddMemberValues, type Member, type Project, type Role } from './schema';
import { useAddMember, useChangeRole, useMembers, useRemoveMember } from './useProjects';

export function ProjectMembersPage() {
  return <ProjectFrame tab="members">{(project) => <Members project={project} />}</ProjectFrame>;
}

function Members({ project }: { project: Project }) {
  const t = useTranslations('members');
  const common = useTranslations('common');
  const query = useMembers(project.id);
  const result = query.data;
  // Owners manage members of an active project; everyone else reads.
  const canManage = project.role === 'owner' && !project.archivedAt;
  const [error, setError] = useState<ActionError | null>(null);

  return (
    <div className="grid gap-6">
      {canManage ? <AddMember project={project} /> : null}
      <Card>
        <CardHeader>
          <CardTitle>{t('title')}</CardTitle>
          <CardDescription>{canManage ? t('description') : t('readOnly')}</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          {error ? <ErrorAlert error={error} /> : null}
          {!result ? (
            <PageSkeleton />
          ) : !result.ok ? (
            <ErrorAlert
              error={result.error}
              title={t('errorTitle')}
              action={
                <Button variant="outline" size="sm" className="mt-2" onClick={() => void query.refetch()}>
                  {common('retry')}
                </Button>
              }
            />
          ) : (
            <ul className="divide-y">
              {result.data.map((member) => (
                <MemberRow
                  key={member.userId}
                  project={project}
                  member={member}
                  canManage={canManage}
                  onError={setError}
                />
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function MemberRow({
  project,
  member,
  canManage,
  onError,
}: {
  project: Project;
  member: Member;
  canManage: boolean;
  onError: (error: ActionError | null) => void;
}) {
  const t = useTranslations('members');
  const roles = useTranslations('roles');
  const common = useTranslations('common');
  const me = useCurrentUser();
  const changeRole = useChangeRole(project.id);
  const remove = useRemoveMember(project.id);
  const isMe = member.userId === me.id;

  async function onRole(role: string) {
    onError(null);
    const result = await changeRole.mutateAsync({ userId: member.userId, role: role as Role });
    if (!result.ok) onError(result.error);
  }

  async function onRemove() {
    onError(null);
    const result = await remove.mutateAsync(member.userId);
    if (!result.ok) onError(result.error);
  }

  return (
    <li className="flex flex-wrap items-center gap-3 py-3">
      <span className="min-w-0 flex-1 truncate">
        {member.email}
        {isMe ? <span className="text-muted-foreground"> ({t('you')})</span> : null}
      </span>
      {canManage ? (
        <>
          <Select value={member.role} onValueChange={(role) => void onRole(role)} disabled={changeRole.isPending}>
            <SelectTrigger className="w-36" aria-label={t('roleFor', { email: member.email })}>
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
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                disabled={remove.isPending}
                aria-label={t('removeFrom', { email: member.email })}
              >
                {remove.isPending ? t('removing') : t('remove')}
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>{t('removeConfirmTitle', { email: member.email })}</AlertDialogTitle>
                <AlertDialogDescription>{t('removeConfirm', { project: project.name })}</AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>{common('cancel')}</AlertDialogCancel>
                <AlertDialogAction onClick={() => void onRemove()}>{t('remove')}</AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        </>
      ) : (
        <RoleBadge role={member.role} />
      )}
    </li>
  );
}

function AddMember({ project }: { project: Project }) {
  const t = useTranslations('members');
  const roles = useTranslations('roles');
  const me = useCurrentUser();
  const add = useAddMember(project.id);
  const invite = useInvite();
  const roleId = useId();
  const [error, setError] = useState<ActionError | null>(null);
  const [added, setAdded] = useState<string | null>(null);
  // What was last tried: an administrator can invite that unknown email and add it.
  const [attempt, setAttempt] = useState<AddMemberValues | null>(null);

  async function inviteAndAdd(values: AddMemberValues) {
    setError(null);
    const invited = await invite.mutateAsync({ email: values.email, locale: me.locale, isAdmin: false });
    if (!invited.ok) {
      setError(invited.error);
      return;
    }
    const result = await add.mutateAsync(values);
    if (result.ok) {
      setAdded(t('invitedAndAdded', { email: result.data.email }));
      setAttempt(null);
      form.reset();
    } else {
      setError(result.error);
    }
  }

  const form = useForm({
    defaultValues: { email: '', role: 'viewer' } as AddMemberValues,
    validators: { onSubmit: addMemberSchema },
    onSubmit: async ({ value, formApi }) => {
      setError(null);
      setAdded(null);
      setAttempt(value);
      const result = await add.mutateAsync(value);
      if (result.ok) {
        setAdded(t('added', { email: result.data.email }));
        formApi.reset();
      } else {
        setError(result.error);
      }
    },
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('addTitle')}</CardTitle>
        <CardDescription>{t('addDescription')}</CardDescription>
      </CardHeader>
      <CardContent>
        <form
          noValidate
          className="grid gap-2"
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
                  {invite.isPending ? t('inviting') : t('inviteAndAdd', { email: attempt.email.trim() })}
                </Button>
              </AlertDescription>
            </Alert>
          ) : error ? (
            <ErrorAlert error={error} />
          ) : null}
          {added ? (
            <Alert variant="success" aria-live="polite">
              <CircleCheck aria-hidden />
              <AlertDescription>{added}</AlertDescription>
            </Alert>
          ) : null}
          <div className="grid gap-2 sm:grid-cols-[1fr_12rem_auto] sm:items-start">
            <form.Field name="email">
              {(field) => <TextField field={field} label={t('email')} type="email" autoComplete="off" required />}
            </form.Field>
            <form.Field name="role">
              {(field) => (
                <div className="grid gap-1.5">
                  <Label htmlFor={roleId}>{t('role')}</Label>
                  <Select value={field.state.value} onValueChange={(role) => field.handleChange(role as Role)}>
                    <SelectTrigger id={roleId}>
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
                  <p className="min-h-5 text-xs text-muted-foreground">{roles(`${field.state.value}Hint`)}</p>
                </div>
              )}
            </form.Field>
            <form.Subscribe selector={(state) => state.isSubmitting}>
              {(isSubmitting) => (
                <Button type="submit" disabled={isSubmitting} className="sm:mt-5.5">
                  {isSubmitting ? t('adding') : t('add')}
                </Button>
              )}
            </form.Subscribe>
          </div>
        </form>
      </CardContent>
    </Card>
  );
}
