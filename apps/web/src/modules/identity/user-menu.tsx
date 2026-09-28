'use client';

import { CircleUser, LogOut } from 'lucide-react';
import { useTranslations } from 'next-intl';

import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { useRouter } from '@/i18n/navigation';

import type { User } from './schema';
import { useSignOut } from './useSession';

export function UserMenu({ user }: { user: User }) {
  const t = useTranslations('common');
  const router = useRouter();
  const signOut = useSignOut();

  async function handleSignOut() {
    const result = await signOut.mutateAsync();
    // Even if the server could not be reached, leave the signed-in pages; the
    // cookie then expires on its own.
    router.replace(result.ok ? '/sign-in?signedOut=1' : '/sign-in');
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" aria-label={t('account')}>
          <CircleUser aria-hidden />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-60">
        <DropdownMenuLabel className="truncate font-normal text-muted-foreground">{user.email}</DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuItem disabled={signOut.isPending} onSelect={() => void handleSignOut()}>
          <LogOut aria-hidden />
          {signOut.isPending ? t('signingOut') : t('signOut')}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
