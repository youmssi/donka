import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { FolderOpen } from 'lucide-react';
import { useEffect } from 'react';

import { Person } from '@/components/shared/person';
import { ShownOnceToken } from '@/components/shared/shown-once-token';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemMedia,
  ItemSeparator,
  ItemTitle,
} from '@/components/ui/item';

import { useStoryMessages } from './fixtures';

function TokenPreview({ denied }: { denied: boolean }) {
  const m = useStoryMessages();
  useEffect(() => {
    if (!denied || !navigator.clipboard) return;
    const clipboard = navigator.clipboard;
    const writeText = clipboard.writeText;
    clipboard.writeText = async () => {
      throw new Error('Clipboard access denied in this preview');
    };
    return () => {
      clipboard.writeText = writeText;
    };
  }, [denied]);
  return (
    <div className="max-w-lg">
      <ShownOnceToken label={m.tokens.namePlaceholder} token="preview-only-not-a-runtime-token" />
    </div>
  );
}

function ContentPreview({ kind }: { kind: 'card' | 'items' | 'person' | 'token' | 'token-error' }) {
  const m = useStoryMessages();
  if (kind === 'token' || kind === 'token-error') return <TokenPreview denied={kind === 'token-error'} />;
  if (kind === 'person')
    return (
      <div className="grid max-w-lg gap-4">
        <Person email="analyst@donka.local" note={<Badge variant="outline">{m.roles.owner}</Badge>} />
        <Person email="long.sample.name.for.the.preview@donka.local" note={m.roles.viewer} />
      </div>
    );
  if (kind === 'items')
    return (
      <ItemGroup className="max-w-2xl">
        <Item>
          <ItemMedia variant="icon">
            <FolderOpen aria-hidden />
          </ItemMedia>
          <ItemContent>
            <ItemTitle>{m.tokens.namePlaceholder}</ItemTitle>
            <ItemDescription>{m.projects.description}</ItemDescription>
          </ItemContent>
          <ItemActions>
            <Button variant="outline">{m.common.save}</Button>
          </ItemActions>
        </Item>
        <ItemSeparator />
        <Item>
          <ItemContent>
            <ItemTitle>{m.releases.title}</ItemTitle>
            <ItemDescription>{m.releases.empty}</ItemDescription>
          </ItemContent>
        </Item>
      </ItemGroup>
    );
  return (
    <Card className="max-w-xl">
      <CardHeader>
        <CardTitle>{m.tokens.namePlaceholder}</CardTitle>
        <CardDescription>{m.projects.description}</CardDescription>
        <CardAction>
          <Badge variant="outline">{m.roles.owner}</Badge>
        </CardAction>
      </CardHeader>
      <CardContent>
        <Person email="analyst@donka.local" />
      </CardContent>
      <CardFooter>
        <Button>{m.common.save}</Button>
      </CardFooter>
    </Card>
  );
}

const meta = { title: 'UI/Content', component: ContentPreview, args: { kind: 'card' } } satisfies Meta<
  typeof ContentPreview
>;
export default meta;
type Story = StoryObj<typeof meta>;
export const CardView: Story = {};
export const Items: Story = { args: { kind: 'items' } };
export const People: Story = { args: { kind: 'person' } };
export const ShownToken: Story = { args: { kind: 'token' } };

export const CopyFailure: Story = { args: { kind: 'token-error' } };
