'use client';

import { Check, ChevronsUpDown, LogOut, Monitor, Moon, Sun } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { useTheme } from 'next-themes';

import { LOCALE_NAMES, useLocaleSwitch } from '@/components/shared/language-switch';
import { initials } from '@/components/shared/person';
import { Avatar, AvatarFallback } from '@/components/ui/avatar';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSkeleton,
  SidebarRail,
  useSidebar,
} from '@/components/ui/sidebar';
import { Link, usePathname, useRouter } from '@/i18n/navigation';
import { routing } from '@/i18n/routing';
import { useCurrentUser, useSignOut } from '@/modules/identity';
import { projectHref, useOpenProject, useProjectList, type Project, type ProjectSection } from '@/modules/project';

import {
  navSection,
  PEOPLE_ICON,
  placeOf,
  PROJECTS_ICON,
  projectSections,
  RUNTIMES_ICON,
  type Place,
} from './sections';

/** Studio's navigation: sections, the open project, and the account. */
export function AppSidebar() {
  const t = useTranslations('nav');
  const app = useTranslations('app');
  const user = useCurrentUser();
  const place = placeOf(usePathname());
  const { setOpenMobile } = useSidebar();
  // On a phone the sidebar is a sheet: close it once a link is followed.
  const close = () => setOpenMobile(false);

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" asChild tooltip={app('name')}>
              <Link href="/" onClick={close}>
                <span
                  aria-hidden
                  className="grid size-8 shrink-0 place-items-center rounded-md bg-sidebar-primary font-semibold text-sidebar-primary-foreground"
                >
                  D
                </span>
                <span className="grid leading-tight">
                  <span className="font-semibold">{app('name')}</span>
                  <span className="text-xs text-muted-foreground">Studio</span>
                </span>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>{t('workspace')}</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={place.kind === 'projects' || place.kind === 'project'}
                  tooltip={t('projects')}
                >
                  <Link href="/" onClick={close}>
                    <PROJECTS_ICON aria-hidden />
                    <span>{t('projects')}</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
              {user.isAdmin ? (
                <>
                  <SidebarMenuItem>
                    <SidebarMenuButton asChild isActive={place.kind === 'people'} tooltip={t('people')}>
                      <Link href="/people" onClick={close}>
                        <PEOPLE_ICON aria-hidden />
                        <span>{t('people')}</span>
                      </Link>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                  <SidebarMenuItem>
                    <SidebarMenuButton asChild isActive={place.kind === 'runtimes'} tooltip={t('runtimes')}>
                      <Link href="/runtimes" onClick={close}>
                        <RUNTIMES_ICON aria-hidden />
                        <span>{t('runtimes')}</span>
                      </Link>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                </>
              ) : null}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
        {place.kind === 'project' ? <OpenProjectGroup place={place} onNavigate={close} /> : null}
      </SidebarContent>

      <SidebarFooter>
        <AccountMenu />
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}

/** The open project: a switcher to another project, then its sections. */
function OpenProjectGroup({
  place,
  onNavigate,
}: {
  place: Extract<Place, { kind: 'project' }>;
  onNavigate: () => void;
}) {
  const t = useTranslations('nav');
  const sections = useTranslations('project');
  const result = useOpenProject().query.data;

  if (!result) {
    return (
      <SidebarGroup>
        <SidebarMenu>
          {[0, 1, 2].map((index) => (
            <SidebarMenuItem key={index}>
              <SidebarMenuSkeleton showIcon />
            </SidebarMenuItem>
          ))}
        </SidebarMenu>
      </SidebarGroup>
    );
  }
  if (!result.ok) return null;
  const project = result.data;

  return (
    <SidebarGroup>
      <SidebarGroupLabel>{t('project')}</SidebarGroupLabel>
      <SidebarGroupContent>
        <SidebarMenu>
          <SidebarMenuItem>
            <ProjectSwitcher project={project} section={place.section} onNavigate={onNavigate} />
          </SidebarMenuItem>
          {projectSections(project).map(({ section, icon: Icon }) => (
            <SidebarMenuItem key={section}>
              <SidebarMenuButton asChild isActive={navSection(place.section) === section} tooltip={sections(section)}>
                <Link href={projectHref(section, project.key)} onClick={onNavigate}>
                  <Icon aria-hidden />
                  <span>{sections(section)}</span>
                </Link>
              </SidebarMenuButton>
            </SidebarMenuItem>
          ))}
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  );
}

function ProjectSwitcher({
  project,
  section,
  onNavigate,
}: {
  project: Project;
  section: ProjectSection;
  onNavigate: () => void;
}) {
  const t = useTranslations('nav');
  const roles = useTranslations('roles');
  const router = useRouter();
  const { isMobile } = useSidebar();
  const list = useProjectList(false, 0).data;
  const others = list?.ok ? list.data.items : [];

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <SidebarMenuButton size="lg" tooltip={project.name} aria-label={t('switchProject', { name: project.name })}>
          <span
            aria-hidden
            className="grid size-8 shrink-0 place-items-center rounded-md border bg-background text-xs font-semibold uppercase"
          >
            {project.key.slice(0, 2)}
          </span>
          <span className="grid min-w-0 flex-1 leading-tight">
            <span className="truncate font-medium">{project.name}</span>
            {/* Your role is about the project, so it sits with the project. */}
            <span className="truncate text-xs text-muted-foreground">
              <span className="font-mono">{project.key}</span> · {roles(project.role)}
            </span>
          </span>
          <ChevronsUpDown className="ml-auto" aria-hidden />
        </SidebarMenuButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent className="w-64" side={isMobile ? 'bottom' : 'right'} align="start">
        <DropdownMenuLabel className="text-xs text-muted-foreground">{t('projects')}</DropdownMenuLabel>
        {others.map((other) => (
          <DropdownMenuItem
            key={other.id}
            onSelect={() => {
              onNavigate();
              // The same section in the other project, when the person may open it there.
              const same = navSection(section);
              const next = same === 'audit' && other.role !== 'owner' ? 'decisions' : same;
              router.push(projectHref(next, other.key));
            }}
          >
            <span className="truncate">{other.name}</span>
            {other.id === project.id ? <Check className="ml-auto" aria-hidden /> : null}
          </DropdownMenuItem>
        ))}
        <DropdownMenuSeparator />
        <DropdownMenuItem asChild>
          <Link href="/" onClick={onNavigate}>
            {t('allProjects')}
          </Link>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/** Who is signed in, with language, theme and sign-out. */
function AccountMenu() {
  const t = useTranslations('common');
  const user = useCurrentUser();
  const router = useRouter();
  const signOut = useSignOut();
  const { isMobile } = useSidebar();
  const { theme, setTheme } = useTheme();
  const { locale, change } = useLocaleSwitch();

  async function handleSignOut() {
    const result = await signOut.mutateAsync();
    // Even if the server could not be reached, leave the signed-in pages; the
    // cookie then expires on its own.
    router.replace(result.ok ? '/sign-in?signedOut=1' : '/sign-in');
  }

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <SidebarMenuButton size="lg" tooltip={user.email} aria-label={t('account')}>
              <Avatar className="size-8 rounded-md">
                <AvatarFallback className="rounded-md text-xs">{initials(user.email)}</AvatarFallback>
              </Avatar>
              <span className="grid min-w-0 flex-1 leading-tight">
                <span className="truncate text-sm">{user.email}</span>
                {user.isAdmin ? <span className="truncate text-xs text-muted-foreground">{t('admin')}</span> : null}
              </span>
              <ChevronsUpDown className="ml-auto" aria-hidden />
            </SidebarMenuButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent className="w-60" side={isMobile ? 'top' : 'right'} align="end">
            <DropdownMenuLabel className="truncate font-normal text-muted-foreground">{user.email}</DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuSub>
              <DropdownMenuSubTrigger>{t('language')}</DropdownMenuSubTrigger>
              <DropdownMenuSubContent>
                <DropdownMenuRadioGroup value={locale} onValueChange={change}>
                  {routing.locales.map((code) => (
                    <DropdownMenuRadioItem key={code} value={code} lang={code}>
                      {t(LOCALE_NAMES[code])}
                    </DropdownMenuRadioItem>
                  ))}
                </DropdownMenuRadioGroup>
              </DropdownMenuSubContent>
            </DropdownMenuSub>
            <DropdownMenuSub>
              <DropdownMenuSubTrigger>{t('theme')}</DropdownMenuSubTrigger>
              <DropdownMenuSubContent>
                <DropdownMenuRadioGroup value={theme ?? 'system'} onValueChange={setTheme}>
                  <DropdownMenuRadioItem value="light">
                    <Sun aria-hidden /> {t('themeLight')}
                  </DropdownMenuRadioItem>
                  <DropdownMenuRadioItem value="dark">
                    <Moon aria-hidden /> {t('themeDark')}
                  </DropdownMenuRadioItem>
                  <DropdownMenuRadioItem value="system">
                    <Monitor aria-hidden /> {t('themeSystem')}
                  </DropdownMenuRadioItem>
                </DropdownMenuRadioGroup>
              </DropdownMenuSubContent>
            </DropdownMenuSub>
            <DropdownMenuSeparator />
            <DropdownMenuItem disabled={signOut.isPending} onSelect={() => void handleSignOut()}>
              <LogOut aria-hidden />
              {signOut.isPending ? t('signingOut') : t('signOut')}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}
