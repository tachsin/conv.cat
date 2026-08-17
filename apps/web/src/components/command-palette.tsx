'use client';

// The "agentsroom"-style quick switcher: press Cmd/Ctrl+K anywhere to jump straight to an action
// instead of hunting for the right button or format picker — same idea as Discord's/VS Code's
// palette, sized to what this app actually needs (a handful of files and a couple of app-level
// actions, not a navigation tree).

import { useEffect, useRef, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { Command } from 'cmdk';
import { AnimatePresence, motion } from 'motion/react';
import { Code2, Download, FilePlus, Moon, RotateCcw, Sun, Trash2, Wand2 } from 'lucide-react';

import type { ConversionJob } from '@/lib/use-converter';
import { formatBytes, outputFileName } from '@/lib/formats';

interface CommandPaletteProps {
  jobs: ConversionJob[];
  convertibleCount: number;
  isDark: boolean;
  onAddFiles: (files: FileList) => void;
  onConvertAll: () => void;
  onConvertJob: (id: string) => void;
  onRemoveJob: (id: string) => void;
  onToggleTheme: () => void;
}

export function CommandPalette({
  jobs,
  convertibleCount,
  isDark,
  onAddFiles,
  onConvertAll,
  onConvertJob,
  onRemoveJob,
  onToggleTheme,
}: CommandPaletteProps) {
  const [open, setOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key.toLowerCase() !== 'k' || !(event.metaKey || event.ctrlKey)) return;

      // Cmd/Ctrl+K is also a native "focus search" shortcut in plenty of form fields — don't
      // hijack it out from under someone who's mid-typing in one. The palette's own search box
      // is exempt, so the same shortcut still closes the palette while it's focused.
      const target = event.target;
      const isEditable =
        target !== searchRef.current &&
        (target instanceof HTMLInputElement ||
          target instanceof HTMLTextAreaElement ||
          (target instanceof HTMLElement && target.isContentEditable));
      if (isEditable) return;

      event.preventDefault();
      setOpen((current) => !current);
    }
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  function run(action: () => void) {
    action();
    setOpen(false);
  }

  function triggerFilePicker() {
    inputRef.current?.click();
  }

  function openSource() {
    window.open('https://github.com/tachsin/conv.cat', '_blank');
  }

  return (
    <>
      <button type="button" className="command-palette-hint" onClick={() => setOpen(true)}>
        <span>Quick actions</span>
        <kbd className="command-palette-kbd">⌘K</kbd>
      </button>

      <Dialog.Root open={open} onOpenChange={setOpen}>
        <AnimatePresence>
          {open && (
            <Dialog.Portal forceMount>
              <Dialog.Overlay asChild>
                <motion.div
                  className="command-palette-overlay"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.15 }}
                />
              </Dialog.Overlay>
              <Dialog.Content
                aria-describedby={undefined}
                className="command-palette-content"
                onOpenAutoFocus={(event) => {
                  event.preventDefault();
                  searchRef.current?.focus();
                }}
                asChild
              >
                <motion.div
                  initial={{ opacity: 0, scale: 0.96, y: -12 }}
                  animate={{ opacity: 1, scale: 1, y: 0 }}
                  exit={{ opacity: 0, scale: 0.96, y: -12 }}
                  transition={{ type: 'spring', stiffness: 420, damping: 32 }}
                >
                  <Dialog.Title className="sr-only">Quick actions</Dialog.Title>
                  <Command loop>
                    <Command.Input
                      ref={searchRef}
                      placeholder="Type a command or search a file…"
                      className="format-picker-search"
                    />
                    <Command.List className="command-palette-list">
                      <Command.Empty className="format-picker-empty">No matches.</Command.Empty>

                      <Command.Group heading="Actions" className="command-palette-group">
                        <Command.Item className="format-picker-item" onSelect={() => run(triggerFilePicker)}>
                          <FilePlus aria-hidden="true" className="h-4 w-4 text-base-content/50" />
                          Add files…
                        </Command.Item>
                        {convertibleCount > 0 && (
                          <Command.Item className="format-picker-item" onSelect={() => run(onConvertAll)}>
                            <Wand2 aria-hidden="true" className="h-4 w-4 text-base-content/50" />
                            Convert all ({convertibleCount})
                          </Command.Item>
                        )}
                        <Command.Item className="format-picker-item" onSelect={() => run(onToggleTheme)}>
                          {isDark ? (
                            <Sun aria-hidden="true" className="h-4 w-4 text-base-content/50" />
                          ) : (
                            <Moon aria-hidden="true" className="h-4 w-4 text-base-content/50" />
                          )}
                          Switch to {isDark ? 'light' : 'dark'} theme
                        </Command.Item>
                        <Command.Item className="format-picker-item" onSelect={() => run(openSource)}>
                          <Code2 aria-hidden="true" className="h-4 w-4 text-base-content/50" />
                          View source on GitHub
                        </Command.Item>
                      </Command.Group>

                      {jobs.length > 0 && (
                        <Command.Group heading="Files" className="command-palette-group">
                          {jobs.map((job) => (
                            <FileCommandItems key={job.id} job={job} onConvert={onConvertJob} onRemove={onRemoveJob} run={run} />
                          ))}
                        </Command.Group>
                      )}
                    </Command.List>
                  </Command>
                </motion.div>
              </Dialog.Content>
            </Dialog.Portal>
          )}
        </AnimatePresence>
      </Dialog.Root>

      <input
        ref={inputRef}
        type="file"
        multiple
        className="sr-only"
        tabIndex={-1}
        aria-hidden="true"
        onChange={(event) => {
          if (event.target.files && event.target.files.length > 0) onAddFiles(event.target.files);
          event.target.value = '';
        }}
      />
    </>
  );
}

interface FileCommandItemsProps {
  job: ConversionJob;
  onConvert: (id: string) => void;
  onRemove: (id: string) => void;
  run: (action: () => void) => void;
}

function FileCommandItems({ job, onConvert, onRemove, run }: FileCommandItemsProps) {
  const canConvert = (job.status === 'ready' || job.status === 'error') && job.from && job.to;
  const canDownload = job.status === 'done' && job.resultUrl && job.to;

  return (
    <>
      {canConvert && (
        <Command.Item
          value={`${job.status === 'error' ? 'retry' : 'convert'} ${job.file.name}`}
          className="format-picker-item"
          onSelect={() => run(() => onConvert(job.id))}
        >
          {job.status === 'error' ? (
            <RotateCcw aria-hidden="true" className="h-4 w-4 text-base-content/50" />
          ) : (
            <Wand2 aria-hidden="true" className="h-4 w-4 text-base-content/50" />
          )}
          <span className="truncate">
            {job.status === 'error' ? 'Retry' : 'Convert'} {job.file.name}
          </span>
        </Command.Item>
      )}
      {canDownload && (
        <Command.Item
          value={`download ${job.file.name}`}
          className="format-picker-item"
          onSelect={() => run(() => window.open(job.resultUrl, '_blank'))}
        >
          <Download aria-hidden="true" className="h-4 w-4 text-base-content/50" />
          <span className="truncate">
            Download {job.to ? outputFileName(job.file.name, job.to) : job.file.name}
            {job.resultBytes !== undefined ? ` — ${formatBytes(job.resultBytes)}` : ''}
          </span>
        </Command.Item>
      )}
      <Command.Item
        value={`remove ${job.file.name}`}
        className="format-picker-item"
        onSelect={() => run(() => onRemove(job.id))}
      >
        <Trash2 aria-hidden="true" className="h-4 w-4 text-error/70" />
        <span className="truncate">Remove {job.file.name}</span>
      </Command.Item>
    </>
  );
}
