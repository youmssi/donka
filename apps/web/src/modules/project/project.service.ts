import { attempt, failure, http, success, type ActionResult } from '@/components/shared/api';

import type {
  AddMemberValues,
  CreateProjectValues,
  Member,
  Project,
  ProjectDetailsValues,
  ProjectList,
  Role,
} from './schema';

export const PAGE_SIZE = 50;

export function listProjects(archived: boolean, offset: number): Promise<ActionResult<ProjectList>> {
  return attempt(async () => {
    const response = await http.get('projects', {
      searchParams: { archived, limit: PAGE_SIZE, offset },
    });
    return response.ok ? success(await response.json<ProjectList>()) : failure(response);
  });
}

export function createProject(values: CreateProjectValues): Promise<ActionResult<Project>> {
  return attempt(async () => {
    const response = await http.post('projects', { json: values });
    return response.ok ? success(await response.json<Project>()) : failure(response);
  });
}

export function getProject(id: string): Promise<ActionResult<Project>> {
  return attempt(async () => {
    const response = await http.get(`projects/${encodeURIComponent(id)}`);
    return response.ok ? success(await response.json<Project>()) : failure(response);
  });
}

export function getProjectByKey(key: string): Promise<ActionResult<Project>> {
  return attempt(async () => {
    const response = await http.get(`projects/by-key/${encodeURIComponent(key)}`);
    return response.ok ? success(await response.json<Project>()) : failure(response);
  });
}

export function updateProject(id: string, values: ProjectDetailsValues): Promise<ActionResult<Project>> {
  return attempt(async () => {
    const response = await http.patch(`projects/${encodeURIComponent(id)}`, { json: values });
    return response.ok ? success(await response.json<Project>()) : failure(response);
  });
}

export function setArchived(id: string, archived: boolean): Promise<ActionResult<Project>> {
  return attempt(async () => {
    const action = archived ? 'archive' : 'restore';
    const response = await http.post(`projects/${encodeURIComponent(id)}/${action}`);
    return response.ok ? success(await response.json<Project>()) : failure(response);
  });
}

export function listMembers(id: string): Promise<ActionResult<Member[]>> {
  return attempt(async () => {
    const response = await http.get(`projects/${encodeURIComponent(id)}/members`);
    return response.ok ? success((await response.json<{ items: Member[] }>()).items) : failure(response);
  });
}

export function addMember(id: string, values: AddMemberValues): Promise<ActionResult<Member>> {
  return attempt(async () => {
    const response = await http.post(`projects/${encodeURIComponent(id)}/members`, { json: values });
    return response.ok ? success(await response.json<Member>()) : failure(response);
  });
}

export function changeRole(id: string, userId: string, role: Role): Promise<ActionResult<Member>> {
  return attempt(async () => {
    const response = await http.patch(`projects/${encodeURIComponent(id)}/members/${encodeURIComponent(userId)}`, {
      json: { role },
    });
    return response.ok ? success(await response.json<Member>()) : failure(response);
  });
}

export function removeMember(id: string, userId: string): Promise<ActionResult<null>> {
  return attempt(async () => {
    const response = await http.delete(`projects/${encodeURIComponent(id)}/members/${encodeURIComponent(userId)}`);
    return response.ok ? success(null) : failure(response);
  });
}
