'use client';

import { useId, useMemo } from 'react';
import type { FormatInfo } from '@conv.cat/engine';

import type { ConversionJob } from '@/lib/use-converter';
import { formatBytes, formatLabel, outputFileName, readableFormats, writableFormatsInCategory } from '@/lib/formats';

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

  const isConverting = job.status === 'converting';
  const canConvert = (job.status === 'ready' || job.status === 'error') && job.from && job.to;

  return (
    <li className={`converter-file-row${isConverting ? ' converter-file-row--converting' : ''}`}>
      <span className="converter-file-thumb converter-file-thumb--empty" aria-hidden="true">
        {statusGlyph(job.status)}
      </span>

      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium text-base-content">{job.file.name}</p>
        <p className="text-xs text-base-content/50">{formatBytes(job.file.size)}</p>

        <div className="mt-2 flex flex-wrap items-center gap-2">
          <label className="sr-only" htmlFor={fromId}>
            Convert {job.file.name} from
          </label>
          <select
            id={fromId}
            className="converter-input select select-sm"
            value={job.from?.id ?? ''}
            disabled={isConverting}
            onChange={(event) => {
              const next = fromOptions.find((format) => format.id === event.target.value);
              if (next) onChangeFrom(next);
            }}
          >
            {!job.from && <option value="">Choose the source format…</option>}
            {fromOptions.map((format) => (
              <option key={format.id} value={format.id}>
                {formatLabel(format)}
              </option>
            ))}
          </select>

          {job.from && (
            <>
              <span aria-hidden="true" className="text-base-content/40">
                →
              </span>
              <label className="sr-only" htmlFor={toId}>
                Convert {job.file.name} to
              </label>
              <select
                id={toId}
                className="converter-input select select-sm"
                value={job.to?.id ?? ''}
                disabled={isConverting}
                onChange={(event) => {
                  const next = toOptions.find((format) => format.id === event.target.value);
                  if (next) onChangeTo(next);
                }}
              >
                {toOptions.map((format) => (
                  <option key={format.id} value={format.id}>
                    {formatLabel(format)}
                  </option>
                ))}
              </select>
            </>
          )}
        </div>

        <p className={`converter-file-status converter-file-status--${statusModifier(job.status)} mt-1.5`}>
          {STATUS_LABEL[job.status]}
          {job.status === 'error' && job.errorMessage ? `: ${job.errorMessage}` : ''}
          {job.status === 'done' && job.resultBytes !== undefined ? ` — ${formatBytes(job.resultBytes)}` : ''}
        </p>

        {isConverting && (
          <div className="mt-1.5">
            <label className="sr-only" htmlFor={progressId}>
              Converting {job.file.name}
            </label>
            <progress
              id={progressId}
              className="converter-progress-track [&::-webkit-progress-value]:bg-primary [&::-webkit-progress-value]:rounded-full [&::-moz-progress-bar]:bg-primary"
              value={job.progress}
              max={1}
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
          <span aria-hidden="true">✕</span>
        </button>
      </div>
    </li>
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

function statusGlyph(status: ConversionJob['status']): string {
  switch (status) {
    case 'done':
      return '✅';
    case 'error':
      return '⚠️';
    case 'converting':
      return '⏳';
    default:
      return '📄';
  }
}
