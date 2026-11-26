import type { Meta, StoryObj } from '@storybook/nextjs-vite';

import { Pager } from '@/components/shared/pager';
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb';
import { Button } from '@/components/ui/button';
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from '@/components/ui/command';
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Kbd, KbdGroup } from '@/components/ui/kbd';
import { Separator } from '@/components/ui/separator';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';

import { useStoryMessages } from './fixtures';

function NavigationPreview({ kind }: { kind: 'tabs' | 'command' | 'menu' | 'pagination' }) {
  const m = useStoryMessages();
  if (kind === 'pagination')
    return <Pager offset={10} pageSize={10} total={35} hrefFor={(offset) => `?offset=${offset}`} />;
  if (kind === 'command')
    return (
      <Command className="max-w-lg rounded-lg border">
        <CommandInput placeholder={m.command.open} />
        <CommandList>
          <CommandEmpty>{m.projects.emptyTitle}</CommandEmpty>
          <CommandGroup heading={m.projects.title}>
            <CommandItem>
              {m.tokens.namePlaceholder}
              <CommandShortcut>
                <KbdGroup>
                  <Kbd>Ctrl</Kbd>
                  <Kbd>K</Kbd>
                </KbdGroup>
              </CommandShortcut>
            </CommandItem>
          </CommandGroup>
          <CommandSeparator />
          <CommandGroup>
            <CommandItem>{m.people.title}</CommandItem>
          </CommandGroup>
        </CommandList>
      </Command>
    );
  if (kind === 'menu')
    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline">{m.common.actions}</Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuLabel>{m.common.theme}</DropdownMenuLabel>
          <DropdownMenuItem>{m.common.themeLight}</DropdownMenuItem>
          <DropdownMenuItem>{m.common.themeDark}</DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuCheckboxItem checked>{m.common.themeSystem}</DropdownMenuCheckboxItem>
        </DropdownMenuContent>
      </DropdownMenu>
    );
  return (
    <div className="grid gap-4">
      <Breadcrumb>
        <BreadcrumbList>
          <BreadcrumbItem>
            <BreadcrumbLink href="#projects">{m.projects.title}</BreadcrumbLink>
          </BreadcrumbItem>
          <BreadcrumbSeparator />
          <BreadcrumbItem>
            <BreadcrumbPage>{m.project.settings}</BreadcrumbPage>
          </BreadcrumbItem>
        </BreadcrumbList>
      </Breadcrumb>
      <Separator />
      <Tabs defaultValue="details">
        <TabsList>
          <TabsTrigger value="details">{m.project.settings}</TabsTrigger>
          <TabsTrigger value="members">{m.members.title}</TabsTrigger>
        </TabsList>
        <TabsContent value="details">{m.project.detailsDescription}</TabsContent>
        <TabsContent value="members">{m.members.readOnly}</TabsContent>
      </Tabs>
    </div>
  );
}

const meta = { title: 'UI/Navigation', component: NavigationPreview, args: { kind: 'tabs' } } satisfies Meta<
  typeof NavigationPreview
>;
export default meta;
type Story = StoryObj<typeof meta>;
export const TabsAndBreadcrumb: Story = {};
export const CommandSearch: Story = { args: { kind: 'command' } };
export const RowMenu: Story = { args: { kind: 'menu' } };
export const Pagination: Story = { args: { kind: 'pagination' } };
