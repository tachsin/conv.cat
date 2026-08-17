'use client';

// A searchable, keyboard-driven replacement for a plain `<select>` — the format catalog only
// grows over time (see lib/formats.ts), and scrolling a native dropdown of 30+ entries is a much
// worse experience than typing a few letters. Built on Radix Popover (positioning, focus trap,
// outside-click/Escape handling) + cmdk (fuzzy filtering, roving-tabindex list) rather than
// hand-rolled — both are the accessible primitive of choice for exactly this "combobox" pattern.

import { useRef, useState } from 'react';
import * as Popover from '@radix-ui/react-popover';
import { Command } from 'cmdk';
import { AnimatePresence, motion } from 'motion/react';
import { Check, ChevronDown } from 'lucide-react';
import type { FormatInfo } from '@conv.cat/engine';

import { formatLabel } from '@/lib/formats';

interface FormatPickerProps {
  id?: string;
  label: string;
  placeholder: string;
  value: FormatInfo | undefined;
  options: FormatInfo[];
  disabled?: boolean;
  onChange: (format: FormatInfo) => void;
}

export function FormatPicker({ id, label, placeholder, value, options, disabled, onChange }: FormatPickerProps) {
  const [open, setOpen] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);

  return (
    <Popover.Root open={open} onOpenChange={(next) => !disabled && setOpen(next)}>
      <Popover.Trigger asChild>
        <button
          id={id}
          type="button"
          aria-label={label}
          aria-haspopup="listbox"
          aria-expanded={open}
          disabled={disabled}
          className="format-picker-trigger"
        >
          {value ? (
            <>
              <span className="format-picker-ext">{value.extensions[0] ?? value.category}</span>
              <span className="truncate">{formatLabel(value)}</span>
            </>
          ) : (
            <span className="text-base-content/45">{placeholder}</span>
          )}
          <ChevronDown aria-hidden="true" className="ml-auto h-3.5 w-3.5 shrink-0 text-base-content/40" />
        </button>
      </Popover.Trigger>

      <AnimatePresence>
        {open && (
          <Popover.Portal forceMount>
            <Popover.Content
              align="start"
              sideOffset={6}
              className="format-picker-content"
              onCloseAutoFocus={(event) => event.preventDefault()}
              onOpenAutoFocus={(event) => {
                event.preventDefault();
                searchRef.current?.focus();
              }}
              asChild
            >
              <motion.div
                initial={{ opacity: 0, scale: 0.96, y: -4 }}
                animate={{ opacity: 1, scale: 1, y: 0 }}
                exit={{ opacity: 0, scale: 0.96, y: -4 }}
                transition={{ duration: 0.14, ease: [0.22, 1, 0.36, 1] }}
              >
                <Command loop>
                  <Command.Input ref={searchRef} placeholder="Search formats…" className="format-picker-search" />
                  <Command.List className="format-picker-list">
                    <Command.Empty className="format-picker-empty">No formats match.</Command.Empty>
                    {options.map((format) => (
                      <Command.Item
                        key={format.id}
                        value={`${formatLabel(format)} ${format.extensions.join(' ')}`}
                        onSelect={() => {
                          onChange(format);
                          setOpen(false);
                        }}
                        className="format-picker-item"
                      >
                        <span className="format-picker-ext">{format.extensions[0] ?? format.category}</span>
                        <span className="truncate">{formatLabel(format)}</span>
                        {value?.id === format.id && (
                          <Check aria-hidden="true" className="ml-auto h-3.5 w-3.5 shrink-0 text-primary" />
                        )}
                      </Command.Item>
                    ))}
                  </Command.List>
                </Command>
              </motion.div>
            </Popover.Content>
          </Popover.Portal>
        )}
      </AnimatePresence>
    </Popover.Root>
  );
}
