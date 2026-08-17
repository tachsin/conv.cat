'use client';

import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { CircleAlert, CircleCheck, FileIcon, ImageOff, Loader2, X } from 'lucide-react';
import type { FormatInfo } from '@conv.cat/engine';

import type { ConversionJob } from '@/lib/use-converter';
import { formatBytes, formatLabel, outputFileName, readableFormats, writableFormatsInCategory } from '@/lib/formats';
import { FormatPicker } from './format-picker';
import { Modal } from './modal';

interface FileRowProps {
  job: ConversionJob;
  formats: FormatInfo[];
  onChangeFrom: (format: FormatInfo) => void;
  onChangeTo: (format: FormatInfo) => void;
  onConvert: () => void;
  onCancel: () => void;
  onRemove: () => void;
}

const STATUS_LABEL: Record<ConversionJob['status'], string> = {
  detecting: 'Detecting…',
  'needs-format': 'Pick a format',
  ready: 'Ready',
  converting: 'Converting…',
  done: 'Done',
  error: 'Failed',
};

export function FileRow({ job, formats, onChangeFrom, onChangeTo, onConvert, onCancel, onRemove }: FileRowProps) {
  const fromId = useId();
  const toId = useId();
  const progressId = useId();

  const fromOptions = useMemo(() => readableFormats(formats), [formats]);
  const toOptions = useMemo(
    () => (job.from ? writableFormatsInCategory(formats, job.from.category) : []),
    [formats, job.from],
  );

  const [errorModalOpen, setErrorModalOpen] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [imageLoadError, setImageLoadError] = useState(false);
  // Tracks the "from" format this row last rendered with, purely so the block below can detect a
  // change and reset `imageLoadError` — the React-docs-recommended "adjust state during render"
  // pattern (react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes)
  // instead of a useEffect, which would cause an extra render pass for what's a same-render fixup.
  const [lastFromId, setLastFromId] = useState(job.from?.id);

  const isConverting = job.status === 'converting';
  const canConvert = (job.status === 'ready' || job.status === 'error') && job.from && job.to;

  // Browser <img> support for these formats is uneven (QOI in particular has none) — rather than
  // hardcoding a per-format allow-list, just attempt the preview and fall back to an icon on
  // `onError`. Reset the error flag if the job's own source format changes underneath it (the
  // "from" picker lets someone correct a wrong guess).
  const isImage = job.from?.category === 'image';
  if (job.from?.id !== lastFromId) {
    setLastFromId(job.from?.id);
    setImageLoadError(false);
  }
  const sourcePreviewUrl = useObjectUrl(isImage ? job.file : undefined);
  const resultPreviewAvailable = job.status === 'done' && job.to?.category === 'image' && Boolean(job.resultUrl);
  const showImageThumb = isImage && Boolean(sourcePreviewUrl) && !imageLoadError;
  const showThumbBadge = job.status === 'converting' || job.status === 'done' || job.status === 'error';

  return (
    <motion.li
      layout
      initial={{ opacity: 0, y: -8, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, scale: 0.96, transition: { duration: 0.16 } }}
      transition={{ type: 'spring', stiffness: 500, damping: 38, mass: 0.6 }}
      className={`converter-file-row${isConverting ? ' converter-file-row--converting' : ''}`}
    >
      <div className="converter-file-thumb-wrap">
        {showImageThumb ? (
          <button
            type="button"
            className="converter-file-thumb converter-file-thumb--image"
            onClick={() => setPreviewOpen(true)}
            aria-label={`Preview ${job.file.name}`}
          >
            {/* eslint-disable-next-line @next/next/no-img-element -- blob: URL from an in-memory
                File; next/image's optimizer only handles static/remote sources, not blob:. */}
            <img
              src={sourcePreviewUrl}
              alt=""
              className="h-full w-full object-cover"
              onError={() => setImageLoadError(true)}
            />
          </button>
        ) : (
          <span className="converter-file-thumb converter-file-thumb--empty" aria-hidden="true">
            <AnimatePresence mode="wait" initial={false}>
              <motion.span
                key={job.status}
                initial={{ opacity: 0, scale: 0.6, rotate: -8 }}
                animate={{ opacity: 1, scale: 1, rotate: 0 }}
                exit={{ opacity: 0, scale: 0.6 }}
                transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}
                className="grid place-items-center"
              >
                {thumbFallbackIcon(job.status, isImage && imageLoadError)}
              </motion.span>
            </AnimatePresence>
          </span>
        )}

        {showImageThumb && (
          <AnimatePresence mode="wait">
            {showThumbBadge && (
              <motion.span
                key={job.status}
                initial={{ opacity: 0, scale: 0.5 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.5 }}
                transition={{ duration: 0.16, ease: [0.22, 1, 0.36, 1] }}
                className="converter-file-thumb-badge"
                aria-hidden="true"
              >
                {statusIcon(job.status, 'h-3 w-3')}
              </motion.span>
            )}
          </AnimatePresence>
        )}
      </div>

      {isImage && (
        <Modal
          open={previewOpen}
          onOpenChange={setPreviewOpen}
          title={job.file.name}
          description={job.from && job.to ? `${formatLabel(job.from)} → ${formatLabel(job.to)}` : undefined}
          footer={
            <>
              <button type="button" className="btn-secondary" onClick={() => setPreviewOpen(false)}>
                Close
              </button>
              {job.status === 'done' && job.resultUrl && job.to && (
                <a
                  className="btn-primary"
                  href={job.resultUrl}
                  download={outputFileName(job.file.name, job.to)}
                >
                  Download
                </a>
              )}
            </>
          }
        >
          <div className={`grid gap-3 ${resultPreviewAvailable ? 'sm:grid-cols-2' : ''}`}>
            <div>
              <p className="mb-1 text-[11px] font-semibold tracking-wide text-base-content/40 uppercase">Original</p>
              {sourcePreviewUrl && (
                // eslint-disable-next-line @next/next/no-img-element -- blob: URL, see above.
                <img
                  src={sourcePreviewUrl}
                  alt={`Original preview of ${job.file.name}`}
                  className="max-h-80 w-full rounded-lg border border-base-content/10 object-contain"
                />
              )}
            </div>
            {resultPreviewAvailable && (
              <div>
                <p className="mb-1 text-[11px] font-semibold tracking-wide text-base-content/40 uppercase">
                  Converted{job.to ? ` (${formatLabel(job.to)})` : ''}
                </p>
                {/* eslint-disable-next-line @next/next/no-img-element -- blob: URL, see above. */}
                <img
                  src={job.resultUrl}
                  alt={`Converted preview of ${job.file.name}`}
                  className="max-h-80 w-full rounded-lg border border-base-content/10 object-contain"
                />
              </div>
            )}
          </div>
        </Modal>
      )}

      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium text-base-content">{job.file.name}</p>
        <p className="text-xs text-base-content/50">{formatBytes(job.file.size)}</p>

        <div className="mt-2 flex flex-wrap items-center gap-2">
          <FormatPicker
            id={fromId}
            label={`Convert ${job.file.name} from`}
            placeholder="Choose the source format…"
            value={job.from}
            options={fromOptions}
            disabled={isConverting}
            onChange={onChangeFrom}
          />

          {job.from && (
            <>
              <span aria-hidden="true" className="text-base-content/40">
                →
              </span>
              <FormatPicker
                id={toId}
                label={`Convert ${job.file.name} to`}
                placeholder="Choose the target format…"
                value={job.to}
                options={toOptions}
                disabled={isConverting}
                onChange={onChangeTo}
              />
            </>
          )}
        </div>

        <AnimatePresence mode="wait" initial={false}>
          {job.status === 'error' ? (
            <motion.button
              key={`${job.status}-${job.errorMessage ?? ''}`}
              type="button"
              onClick={() => setErrorModalOpen(true)}
              initial={{ opacity: 0, y: -2 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.15 }}
              className="converter-file-status converter-file-status--error mt-1.5 line-clamp-1 text-left underline decoration-error/40 underline-offset-2 hover:decoration-error"
            >
              {STATUS_LABEL.error}
              {job.errorMessage ? `: ${job.errorMessage}` : ''}
            </motion.button>
          ) : (
            <motion.p
              key={job.status}
              initial={{ opacity: 0, y: -2 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.15 }}
              className={`converter-file-status converter-file-status--${statusModifier(job.status)} mt-1.5`}
            >
              {STATUS_LABEL[job.status]}
              {job.status === 'done' && job.resultBytes !== undefined ? ` — ${formatBytes(job.resultBytes)}` : ''}
            </motion.p>
          )}
        </AnimatePresence>

        {job.status === 'error' && (
          <Modal
            open={errorModalOpen}
            onOpenChange={setErrorModalOpen}
            title="Conversion failed"
            description={job.file.name}
            footer={
              <>
                <button type="button" className="btn-secondary" onClick={() => setErrorModalOpen(false)}>
                  Close
                </button>
                {canConvert && (
                  <button
                    type="button"
                    className="btn-primary"
                    onClick={() => {
                      setErrorModalOpen(false);
                      onConvert();
                    }}
                  >
                    Retry
                  </button>
                )}
              </>
            }
          >
            {job.errorMessage ?? 'Something went wrong during conversion.'}
          </Modal>
        )}

        {isConverting && (
          <div className="converter-progress-track mt-1.5">
            <label className="sr-only" htmlFor={progressId}>
              Converting {job.file.name}
            </label>
            <motion.div
              id={progressId}
              role="progressbar"
              aria-valuenow={Math.round(job.progress * 100)}
              aria-valuemin={0}
              aria-valuemax={100}
              className="converter-progress-fill"
              animate={{ width: `${Math.max(job.progress, 0.04) * 100}%` }}
              transition={{ type: 'spring', stiffness: 300, damping: 30 }}
            />
          </div>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-1.5">
        {isConverting && (
          <button type="button" className="btn-secondary" onClick={onCancel}>
            Cancel
          </button>
        )}
        {canConvert && (
          <button type="button" className="btn-primary" onClick={onConvert}>
            {job.status === 'error' ? 'Retry' : 'Convert'}
          </button>
        )}
        {job.status === 'done' && job.resultUrl && job.to && (
          <a className="btn-primary" href={job.resultUrl} download={outputFileName(job.file.name, job.to)}>
            Download
          </a>
        )}
        <button
          type="button"
          className="converter-file-remove"
          onClick={onRemove}
          aria-label={`Remove ${job.file.name}`}
        >
          <X aria-hidden="true" className="h-3.5 w-3.5" />
        </button>
      </div>
    </motion.li>
  );
}

function statusModifier(status: ConversionJob['status']): string {
  switch (status) {
    case 'ready':
      return 'ready';
    case 'done':
      return 'done';
    case 'converting':
      return 'active';
    case 'error':
      return 'error';
    default:
      return 'active';
  }
}

function statusIcon(status: ConversionJob['status'], sizeClassName = 'h-5 w-5') {
  switch (status) {
    case 'done':
      return <CircleCheck aria-hidden="true" className={`${sizeClassName} text-emerald-500`} />;
    case 'error':
      return <CircleAlert aria-hidden="true" className={`${sizeClassName} text-error`} />;
    case 'converting':
      return <Loader2 aria-hidden="true" className={`${sizeClassName} animate-spin text-primary`} />;
    default:
      return <FileIcon aria-hidden="true" className={`${sizeClassName} text-base-content/40`} />;
  }
}

/** The idle-state thumb icon: a plain file glyph normally, but `ImageOff` specifically for an
 * image job whose preview `<img>` failed to load (QOI has no browser decoder — see
 * `showImageThumb`'s comment) so the empty state hints at *why* there's no preview instead of
 * looking identical to a non-image file. Only for the idle statuses — once conversion is
 * underway or has a result/error, that status is more useful than the "no preview" hint. */
function thumbFallbackIcon(status: ConversionJob['status'], showImageOff: boolean) {
  if (showImageOff && (status === 'ready' || status === 'detecting' || status === 'needs-format')) {
    return <ImageOff aria-hidden="true" className="h-5 w-5 text-base-content/30" />;
  }
  return statusIcon(status);
}

/** Object URL for a `Blob`, revoked automatically when the blob changes or the component
 * unmounts — the same lifecycle `useConverter` already manages for conversion results, just
 * scoped to a single row's own source-file preview instead of the shared job list.
 *
 * The URL is derived with `useMemo`, not `useState`+effect: it's a pure function of `blob`, so
 * there's nothing to "synchronize" on mount — computing it during render avoids the extra render
 * pass a setState-in-effect would cost. The effect's only job is the cleanup `useMemo` can't do:
 * revoking the URL once nothing needs it any more.
 *
 * The revoke is deferred by a tick, not immediate, and cancellable: React's Strict Mode
 * intentionally fires this effect's cleanup-then-setup twice on mount to surface exactly this
 * class of bug — an immediate synchronous revoke would kill the URL microseconds after the
 * `<img>` below started loading it, well before the load completes, breaking every preview in
 * dev. Deferring means the very next setup for the *same* URL (Strict Mode's synthetic re-run,
 * or a real remount) can cancel the pending revoke; a setup for a *different* URL (the blob
 * actually changed) lets the old one's revoke fire on schedule. */
function useObjectUrl(blob: Blob | undefined): string | undefined {
  const url = useMemo(() => (blob ? URL.createObjectURL(blob) : undefined), [blob]);
  const pendingRevoke = useRef<{ url: string; timeoutId: ReturnType<typeof setTimeout> } | null>(null);

  useEffect(() => {
    if (pendingRevoke.current !== null && pendingRevoke.current.url === url) {
      clearTimeout(pendingRevoke.current.timeoutId);
      pendingRevoke.current = null;
    }
    return () => {
      if (!url) return;
      const timeoutId = setTimeout(() => URL.revokeObjectURL(url), 0);
      pendingRevoke.current = { url, timeoutId };
    };
  }, [url]);

  return url;
}
