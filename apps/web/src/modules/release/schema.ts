import { z } from 'zod';

import type { ApiSchemas } from '@/components/shared/api';
import openapi from '../../../openapi.json';

export type ReleaseSummary = ApiSchemas['ReleaseSummaryResponse'];
export type Release = ApiSchemas['ReleaseResponse'];
export type ReleaseList = ApiSchemas['ReleaseListResponse'];
export type ReleasePreview = ApiSchemas['ReleasePreviewResponse'];
export type ReleasedDecision = ApiSchemas['ReleasedDecisionResponse'];
export type Bump = ApiSchemas['BumpRequest'];
export type EnvironmentName = ApiSchemas['EnvironmentName'];
export type EnvironmentState = ApiSchemas['EnvironmentResponse'];
export type Deployment = ApiSchemas['DeploymentResponse'];
export type DeploymentStatus = ApiSchemas['DeploymentStatusResponse'];
export type RuntimeToken = ApiSchemas['TokenResponse'];
export type IssuedToken = ApiSchemas['IssuedTokenResponse'];
export type CiToken = ApiSchemas['CiTokenResponse'];
export type IssuedCiToken = ApiSchemas['IssuedCiTokenResponse'];
export type Approval = ApiSchemas['ApprovalResponse'];
export type ApprovalList = ApiSchemas['ApprovalListResponse'];
export type ApprovalReview = ApiSchemas['ApprovalReviewResponse'];
export type ApprovalStatus = ApiSchemas['ApprovalStatusResponse'];
export type DecisionChange = ApiSchemas['DecisionChangeResponse'];

// Limits come from the API contract, so the forms and the server never disagree.
export const NOTES_MAX = openapi.components.schemas.CreateReleaseRequest.properties.notes.maxLength;
export const TOKEN_NAME_MAX = openapi.components.schemas.IssueTokenRequest.properties.name.maxLength;
export const REASON_MAX = openapi.components.schemas.RejectRequest.properties.reason.maxLength;

export const releaseSchema = z.object({
  bump: z.enum(['major', 'minor', 'patch']),
  notes: z.string().trim().min(1, 'required').max(NOTES_MAX, 'maxLength'),
});
export type ReleaseValues = z.infer<typeof releaseSchema>;

export const tokenSchema = z.object({
  name: z.string().trim().min(1, 'required').max(TOKEN_NAME_MAX, 'maxLength'),
});
export type TokenValues = z.infer<typeof tokenSchema>;

export const rejectSchema = z.object({
  reason: z.string().trim().min(1, 'required').max(REASON_MAX, 'maxLength'),
});
export type RejectValues = z.infer<typeof rejectSchema>;

export const rollbackSchema = z.object({
  releaseId: z.string().min(1, 'required'),
  reason: z.string().trim().min(1, 'required').max(REASON_MAX, 'maxLength'),
});
export type RollbackValues = z.infer<typeof rollbackSchema>;

/** A deployment still on its way to the bucket: the page keeps asking how it went. */
export function inFlight(deployment: Deployment | null | undefined): boolean {
  return deployment?.status === 'pending' || deployment?.status === 'retrying';
}
