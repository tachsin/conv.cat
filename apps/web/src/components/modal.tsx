'use client';

// The one generic modal primitive for this app — Radix Dialog for behavior (focus trap, Escape,
// outside-click, portal), Motion for the backdrop fade + spring-in card. Every future dialog
// (error detail today, settings later) should compose this rather than hand-rolling another
// Dialog.Root/Overlay/Content triplet.

import type { ReactNode } from 'react';
import { useRef } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { AnimatePresence, motion } from 'motion/react';
import { X } from 'lucide-react';

interface ModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
}

export function Modal({ open, onOpenChange, title, description, children, footer }: ModalProps) {
  const closeRef = useRef<HTMLButtonElement>(null);

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <AnimatePresence>
        {open && (
          <Dialog.Portal forceMount>
            <Dialog.Overlay asChild>
              <motion.div
                className="modal-overlay"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.15 }}
              />
            </Dialog.Overlay>
            <Dialog.Content
              className="modal-content"
              onOpenAutoFocus={(event) => {
                event.preventDefault();
                closeRef.current?.focus();
              }}
              asChild
            >
              <motion.div
                initial={{ opacity: 0, scale: 0.95, y: 10 }}
                animate={{ opacity: 1, scale: 1, y: 0 }}
                exit={{ opacity: 0, scale: 0.96, y: 6 }}
                transition={{ type: 'spring', stiffness: 420, damping: 32 }}
              >
                <div className="modal-header">
                  <Dialog.Title className="modal-title">{title}</Dialog.Title>
                  <Dialog.Close ref={closeRef} className="modal-close" aria-label="Close">
                    <X aria-hidden="true" className="h-4 w-4" />
                  </Dialog.Close>
                </div>
                {description && <Dialog.Description className="modal-description">{description}</Dialog.Description>}
                <div className="modal-body">{children}</div>
                {footer && <div className="modal-footer">{footer}</div>}
              </motion.div>
            </Dialog.Content>
          </Dialog.Portal>
        )}
      </AnimatePresence>
    </Dialog.Root>
  );
}
