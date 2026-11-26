import type { Meta, StoryObj } from '@storybook/nextjs-vite';

import { Button } from '@/components/ui/button';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle, SheetTrigger } from '@/components/ui/sheet';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';

import { useStoryMessages } from './fixtures';

function OverlayPreview({ kind }: { kind: 'dialog' | 'confirm' | 'sheet' | 'popover' | 'tooltip' }) {
  const m = useStoryMessages();
  if (kind === 'confirm')
    return (
      <AlertDialog>
        <AlertDialogTrigger asChild>
          <Button variant="destructive">{m.project.archive}</Button>
        </AlertDialogTrigger>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{m.project.archiveTitle}</AlertDialogTitle>
            <AlertDialogDescription>{m.project.archiveDescription}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{m.common.cancel}</AlertDialogCancel>
            <AlertDialogAction>{m.project.archive}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    );
  if (kind === 'sheet')
    return (
      <Sheet>
        <SheetTrigger asChild>
          <Button>{m.inputFields.open}</Button>
        </SheetTrigger>
        <SheetContent>
          <SheetHeader>
            <SheetTitle>{m.inputFields.title}</SheetTitle>
            <SheetDescription>{m.inputFields.description}</SheetDescription>
          </SheetHeader>
        </SheetContent>
      </Sheet>
    );
  if (kind === 'popover')
    return (
      <Popover>
        <PopoverTrigger asChild>
          <Button>{m.projects.description}</Button>
        </PopoverTrigger>
        <PopoverContent>
          <p>{m.projects.emptyAdmin}</p>
        </PopoverContent>
      </Popover>
    );
  if (kind === 'tooltip')
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <Button variant="outline">{m.common.theme}</Button>
        </TooltipTrigger>
        <TooltipContent>{m.common.themeSystem}</TooltipContent>
      </Tooltip>
    );
  return (
    <Dialog>
      <DialogTrigger asChild>
        <Button>{m.projects.create}</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{m.projects.createTitle}</DialogTitle>
          <DialogDescription>{m.projects.createDescription}</DialogDescription>
        </DialogHeader>
        <DialogFooter showCloseButton />
      </DialogContent>
    </Dialog>
  );
}

const meta = { title: 'UI/Overlays', component: OverlayPreview, args: { kind: 'dialog' } } satisfies Meta<
  typeof OverlayPreview
>;
export default meta;
type Story = StoryObj<typeof meta>;
export const DialogView: Story = {};
export const Confirmation: Story = { args: { kind: 'confirm' } };
export const SideSheet: Story = { args: { kind: 'sheet' } };
export const PopoverView: Story = { args: { kind: 'popover' } };
export const TooltipView: Story = { args: { kind: 'tooltip' } };
