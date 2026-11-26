import type { ReactNode } from 'react';

import { Avatar, AvatarFallback } from '@/components/ui/avatar';

/** Two letters for an avatar: "grace.hopper@…" gives "GH", "ada@…" gives "AD". */
export function initials(email: string): string {
  const name = email.split('@')[0] ?? '';
  const parts = name.split(/[._-]+/).filter(Boolean);
  const letters = parts.length > 1 ? `${parts[0]?.[0] ?? ''}${parts[1]?.[0] ?? ''}` : name.slice(0, 2);
  return letters.toUpperCase();
}

/** A person in a list: avatar with initials, their email, and an optional note. */
export function Person({ email, note }: { email: string; note?: ReactNode }) {
  return (
    <span className="flex min-w-0 items-center gap-2">
      <Avatar className="size-7">
        <AvatarFallback className="text-xs">{initials(email)}</AvatarFallback>
      </Avatar>
      <span className="truncate">{email}</span>
      {note ? <span className="shrink-0 text-muted-foreground">{note}</span> : null}
    </span>
  );
}
