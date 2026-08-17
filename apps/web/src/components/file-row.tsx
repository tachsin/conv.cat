'use client';

import { useId, useMemo, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { CircleAlert, CircleCheck, FileIcon, Loader2, X } from 'lucide-react';
import type { FormatInfo } from '@conv.cat/engine';

import type { ConversionJob } from '@/lib/use-converter';
import { formatBytes, outputFileName, readableFormats, writableFormatsInCategory } from '@/lib/formats';
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

  const isConverting = job.status === 'converting';
  const canConvert = (job.status === 'ready' || job.status === 'error') && job.from && job.to;

  return (
    <motion.li
      layout
      initial={{ opacity: 0, y: -8, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, scale: 0.96, transition: { duration: 0.16 } }}
      transition={{ type: 'spring', stiffness: 500, damping: 38, mass: 0.6 }}
      className={`converter-file-row${isConverting ? ' converter-file-row--converting' : ''}`}
    >
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
            {statusIcon(job.status)}
          </motion.span>
        </AnimatePresence>
      </span>

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

function statusIcon(status: ConversionJob['status']) {
  switch (status) {
    case 'done':
      return <CircleCheck aria-hidden="true" className="h-5 w-5 text-emerald-500" />;
    case 'error':
      return <CircleAlert aria-hidden="true" className="h-5 w-5 text-error" />;
    case 'converting':
      return <Loader2 aria-hidden="true" className="h-5 w-5 animate-spin text-primary" />;
    default:
      return <FileIcon aria-hidden="true" className="h-5 w-5 text-base-content/40" />;
  }
}
