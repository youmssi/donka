import type { Meta, StoryObj } from '@storybook/nextjs-vite';

import { BrandMark } from '@/components/shared/brand-mark';
import { AppShell } from '@/components/shared/layout/app-shell';
import { Brand } from '@/components/shared/layout/brand';
import { CenteredColumn } from '@/components/shared/layout/centered-column';
import { PageHeader } from '@/components/shared/layout/page-header';
import { LanguageSwitch } from '@/components/shared/language-switch';
import { ThemeToggle } from '@/components/shared/theme-toggle';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';

import { useStoryMessages } from './fixtures';

function LayoutPreview({ kind }: { kind: 'brand' | 'page' | 'shell' }) {
  const m = useStoryMessages();
  if (kind === 'shell')
    return (
      <AppShell>
        <CenteredColumn>
          <Card>
            <CardContent>{m.projects.emptyAdmin}</CardContent>
          </Card>
        </CenteredColumn>
      </AppShell>
    );
  if (kind === 'page')
    return (
      <PageHeader
        title={m.projects.title}
        description={m.projects.emptyAdmin}
        badges={<Badge variant="outline">{m.roles.owner}</Badge>}
        actions={<Button>{m.projects.create}</Button>}
      />
    );
  return (
    <div className="flex flex-wrap items-center gap-4">
      <BrandMark className="size-12 bg-primary text-primary-foreground" />
      <Brand />
      <LanguageSwitch />
      <ThemeToggle />
    </div>
  );
}

const meta = { title: 'Shared/Layout', component: LayoutPreview, args: { kind: 'brand' } } satisfies Meta<
  typeof LayoutPreview
>;
export default meta;
type Story = StoryObj<typeof meta>;
export const BrandingAndPreferences: Story = {};
export const PageHeading: Story = { args: { kind: 'page' } };
export const PublicShell: Story = { args: { kind: 'shell' }, parameters: { layout: 'fullscreen' } };
