'use client';

import { MoreHorizontal, UserMinus, Users } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useState } from 'react';
import { toast } from 'sonner';

import type { ActionError } from '@/components/shared/api';
import { DataTable, type DataTableColumn } from '@/components/shared/data-table';
import { ErrorAlert } from '@/components/shared/error-alert';
import { When } from '@/components/shared/format';
import { Person } from '@/components/shared/person';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { useCurrentUser } from '@/modules/identity';

import { AddMemberDialog } from './add-member-dialog';
import { ProjectFrame } from './project-frame';
import { RoleBadge } from './role-badge';
import { ROLES, type Member, type Project, type Role } from './schema';
import { useChangeRole, useMembers, useRemoveMember } from './useProjects';

export function ProjectMembersPage() {
  return (
    <ProjectFrame
      section="members"
      // Owners manage members of an active project; everyone else reads.
      actions={(project) => (canManage(project) ? <AddMemberDialog project={project} /> : null)}
    >
      {(project) => <Members project={project} />}
    </ProjectFrame>
  );
}

function canManage(project: Project): boolean {
  return project.role === 'owner' && !project.archivedAt;
}

function Members({ project }: { project: Project }) {
  const t = useTranslations('members');
  const common = useTranslations('common');
  const me = useCurrentUser();
  const query = useMembers(project.id);
  const result = query.data;
  const [error, setError] = useState<ActionError | null>(null);
  const manage = canManage(project);

  if (result && !result.ok) {
    return (
      <ErrorAlert
        error={result.error}
        title={t('errorTitle')}
        action={
          <Button variant="outline" size="sm" className="mt-2" onClick={() => void query.refetch()}>
            {common('retry')}
          </Button>
        }
      />
    );
  }

  const columns: DataTableColumn<Member>[] = [
    {
      id: 'member',
      header: t('member'),
      cell: ({ row }) => (
        <Person email={row.original.email} note={row.original.userId === me.id ? `(${t('you')})` : undefined} />
      ),
      // Takes the room left, and truncates the email on a phone so the actions stay visible.
      meta: { className: 'w-full max-w-0' },
    },
    { id: 'role', header: t('role'), cell: ({ row }) => <RoleBadge role={row.original.role} /> },
    {
      id: 'added',
      header: t('addedAt'),
      cell: ({ row }) => <When value={row.original.addedAt} />,
      meta: { className: 'hidden text-muted-foreground sm:table-cell' },
    },
    ...(manage
      ? [
          {
            id: 'actions',
            header: () => <span className="sr-only">{common('actions')}</span>,
            cell: ({ row }) => <MemberActions project={project} member={row.original} onError={setError} />,
            meta: { className: 'w-12 text-right' },
          } satisfies DataTableColumn<Member>,
        ]
      : []),
  ];

  return (
    <div className="grid gap-4">
      {manage ? null : <p className="text-sm text-muted-foreground">{t('readOnly')}</p>}
      {error ? <ErrorAlert error={error} /> : null}
      <DataTable
        label={t('title')}
        columns={columns}
        data={result?.data}
        getRowId={(member) => member.userId}
        empty={
          <Empty className="border">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <Users />
              </EmptyMedia>
              <EmptyTitle>{t('emptyTitle')}</EmptyTitle>
              <EmptyDescription>{t('empty')}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        }
      />
    </div>
  );
}

function MemberActions({
  project,
  member,
  onError,
}: {
  project: Project;
  member: Member;
  onError: (error: ActionError | null) => void;
}) {
  const t = useTranslations('members');
  const roles = useTranslations('roles');
  const common = useTranslations('common');
  const changeRole = useChangeRole(project.id);
  const remove = useRemoveMember(project.id);
  const [confirming, setConfirming] = useState(false);

  async function onRole(role: string) {
    if (role === member.role) return;
    onError(null);
    const result = await changeRole.mutateAsync({ userId: member.userId, role: role as Role });
    if (result.ok) toast.success(t('roleChanged', { email: member.email, role: roles(role as Role) }));
    else onError(result.error);
  }

  async function onRemove() {
    onError(null);
    const result = await remove.mutateAsync(member.userId);
    if (result.ok) toast.success(t('removed', { email: member.email }));
    else onError(result.error);
  }

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon-sm" aria-label={t('actionsFor', { email: member.email })}>
            <MoreHorizontal aria-hidden />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-52">
          <DropdownMenuLabel>{t('role')}</DropdownMenuLabel>
          <DropdownMenuRadioGroup value={member.role} onValueChange={(role) => void onRole(role)}>
            {ROLES.map((role) => (
              <DropdownMenuRadioItem key={role} value={role} disabled={changeRole.isPending}>
                {roles(role)}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
          <DropdownMenuSeparator />
          <DropdownMenuItem variant="destructive" onSelect={() => setConfirming(true)}>
            <UserMinus aria-hidden />
            {t('remove')}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <AlertDialog open={confirming} onOpenChange={setConfirming}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('removeConfirmTitle', { email: member.email })}</AlertDialogTitle>
            <AlertDialogDescription>{t('removeConfirm', { project: project.name })}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{common('cancel')}</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={() => void onRemove()}>
              {t('remove')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
