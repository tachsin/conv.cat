'use client';

import { useId, useRef, useState } from 'react';
import type { DragEvent, KeyboardEvent } from 'react';

interface DropZoneProps {
  onFiles: (files: FileList) => void;
  disabled?: boolean;
  disabledReason?: string;
}

/**
 * A single tab stop that is both a click target and a drop target. The real `<input
 * type="file">` stays in the DOM (a hidden input is required — there is no other way to open the
 * OS file picker) but is taken fully out of the accessibility tree and tab order
 * (`aria-hidden`/`tabIndex={-1}`), so a keyboard or screen-reader user only ever encounters the
 * one labeled, `role="button"` element. This is the same pattern most accessible custom
 * drop-zones use (e.g. react-dropzone's own guidance) — a native `<label>`+`<input>` pair would
 * put the tiny native input itself in the tab order instead of this element, which is both a
 * worse target and unstyled by the drop-zone visuals.
 */
export function DropZone({ onFiles, disabled = false, disabledReason }: DropZoneProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const dragDepth = useRef(0);
  const [isActive, setIsActive] = useState(false);
  const hintId = useId();

  function openPicker() {
    if (disabled) return;
    inputRef.current?.click();
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Enter' || event.key === ' ') {
      // Space would otherwise scroll the page — this element is acting as a button.
      event.preventDefault();
      openPicker();
    }
  }

  function handleDragEnter(event: DragEvent<HTMLDivElement>) {
    event.preventDefault();
    if (disabled) return;
    dragDepth.current += 1;
    setIsActive(true);
  }

  function handleDragOver(event: DragEvent<HTMLDivElement>) {
    // Required for onDrop to fire at all — the browser default is to reject a drop.
    event.preventDefault();
  }

  function handleDragLeave(event: DragEvent<HTMLDivElement>) {
    event.preventDefault();
    dragDepth.current = Math.max(0, dragDepth.current - 1);
    if (dragDepth.current === 0) setIsActive(false);
  }

  function handleDrop(event: DragEvent<HTMLDivElement>) {
    event.preventDefault();
    dragDepth.current = 0;
    setIsActive(false);
    if (disabled) return;
    if (event.dataTransfer.files.length > 0) onFiles(event.dataTransfer.files);
  }

  return (
    <div
      className={`drop-zone drop-zone--fill${isActive ? ' drop-zone--active' : ''}`}
      role="button"
      tabIndex={disabled ? -1 : 0}
      aria-disabled={disabled}
      aria-label="Add files to convert"
      aria-describedby={hintId}
      onClick={openPicker}
      onKeyDown={handleKeyDown}
      onDragEnter={handleDragEnter}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      <span className="drop-zone-icon" aria-hidden="true">
        📂
      </span>
      <div className="drop-zone-body">
        <span className="drop-zone-title">Drag files here, or press Enter to browse</span>
        <span className="drop-zone-hint" id={hintId}>
          {disabled
            ? (disabledReason ?? 'Not ready yet.')
            : 'Convert as many files as you like — everything runs on this device, nothing is uploaded.'}
        </span>
      </div>
      <input
        ref={inputRef}
        type="file"
        multiple
        className="sr-only"
        tabIndex={-1}
        aria-hidden="true"
        disabled={disabled}
        onChange={(event) => {
          if (event.target.files && event.target.files.length > 0) {
            onFiles(event.target.files);
          }
          // Reset so selecting the exact same file again still fires onChange.
          event.target.value = '';
        }}
      />
    </div>
  );
}
