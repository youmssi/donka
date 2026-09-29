'use client';

import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { ActionResult } from '@/components/shared/api';

import {
  addMember,
  changeRole,
  createProject,
  getProject,
  listMembers,
  listProjects,
  removeMember,
  setArchived,
  updateProject,
} from './project.service';
import type { AddMemberValues, CreateProjectValues, Project, ProjectDetailsValues, Role } from './schema';

const keys = {
  lists: ['projects'] as const,
  list: (archived: boolean, offset: number) => ['projects', { archived, offset }] as const,
  project: (id: string) => ['project', id] as const,
  members: (id: string) => ['project', id, 'members'] as const,
};

export function useProjectList(archived: boolean, offset: number) {
  return useQuery({
    queryKey: keys.list(archived, offset),
    queryFn: () => listProjects(archived, offset),
    placeholderData: keepPreviousData,
  });
}

export function useProject(id: string) {
  return useQuery({ queryKey: keys.project(id), queryFn: () => getProject(id), enabled: id !== '' });
}

export function useMembers(id: string) {
  return useQuery({ queryKey: keys.members(id), queryFn: () => listMembers(id), enabled: id !== '' });
}

/** After a change to one project, its page and every list showing it are refreshed. */
function useProjectChanged(id: string) {
  const queryClient = useQueryClient();
  return (result: ActionResult<Project>) => {
    if (!result.ok) return;
    queryClient.setQueryData(keys.project(id), result);
    void queryClient.invalidateQueries({ queryKey: keys.lists });
  };
}

export function useCreateProject() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (values: CreateProjectValues) => createProject(values),
    onSuccess: (result) => {
      if (result.ok) void queryClient.invalidateQueries({ queryKey: keys.lists });
    },
  });
}

export function useUpdateProject(id: string) {
  const changed = useProjectChanged(id);
  return useMutation({
    mutationFn: (values: ProjectDetailsValues) => updateProject(id, values),
    onSuccess: changed,
  });
}

export function useSetArchived(id: string) {
  const changed = useProjectChanged(id);
  return useMutation({ mutationFn: (archived: boolean) => setArchived(id, archived), onSuccess: changed });
}

function useMembersChanged(id: string) {
  const queryClient = useQueryClient();
  return (result: ActionResult<unknown>) => {
    if (result.ok) void queryClient.invalidateQueries({ queryKey: keys.members(id) });
    // Your own role may have changed: the project page shows it.
    if (result.ok) void queryClient.invalidateQueries({ queryKey: keys.project(id), exact: true });
  };
}

export function useAddMember(id: string) {
  const changed = useMembersChanged(id);
  return useMutation({ mutationFn: (values: AddMemberValues) => addMember(id, values), onSuccess: changed });
}

export function useChangeRole(id: string) {
  const changed = useMembersChanged(id);
  return useMutation({
    mutationFn: ({ userId, role }: { userId: string; role: Role }) => changeRole(id, userId, role),
    onSuccess: changed,
  });
}

export function useRemoveMember(id: string) {
  const changed = useMembersChanged(id);
  return useMutation({ mutationFn: (userId: string) => removeMember(id, userId), onSuccess: changed });
}
