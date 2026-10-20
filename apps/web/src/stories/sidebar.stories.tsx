import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { FolderOpen, Users } from 'lucide-react';

import { Brand } from '@/components/shared/layout/brand';
import { Person } from '@/components/shared/person';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSkeleton,
  SidebarProvider,
  SidebarRail,
  SidebarTrigger,
} from '@/components/ui/sidebar';

import { useStoryMessages } from './fixtures';

function SidebarPreview({ loading }: { loading: boolean }) {
  const m = useStoryMessages();
  return (
    <SidebarProvider>
      <Sidebar collapsible="icon">
        <SidebarHeader>
          <Brand />
        </SidebarHeader>
        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupLabel>{m.nav.workspace}</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {loading ? (
                  <>
                    <SidebarMenuSkeleton showIcon />
                    <SidebarMenuSkeleton showIcon />
                  </>
                ) : (
                  <>
                    <SidebarMenuItem>
                      <SidebarMenuButton isActive>
                        <FolderOpen aria-hidden />
                        <span>{m.projects.title}</span>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                      <SidebarMenuButton>
                        <Users aria-hidden />
                        <span>{m.people.title}</span>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  </>
                )}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
        <SidebarFooter>
          <Person email="analyst@donka.local" />
        </SidebarFooter>
        <SidebarRail />
      </Sidebar>
      <SidebarInset>
        <header className="flex items-center gap-2 border-b p-3">
          <SidebarTrigger aria-label={m.ui.toggleSidebar} />
          <span>{m.projects.title}</span>
        </header>
        <main className="p-4">{m.projects.emptyAdmin}</main>
      </SidebarInset>
    </SidebarProvider>
  );
}

const meta = {
  title: 'UI/Sidebar',
  component: SidebarPreview,
  args: { loading: false },
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof SidebarPreview>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Success: Story = {};
export const Loading: Story = { args: { loading: true } };
