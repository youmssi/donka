import { useTranslations } from 'next-intl';

import { Badge } from '@/components/ui/badge';

import type { Role } from './schema';

export function RoleBadge({ role }: { role: Role }) {
  const t = useTranslations('roles');
  return <Badge variant={role === 'owner' ? 'default' : 'secondary'}>{t(role)}</Badge>;
}
