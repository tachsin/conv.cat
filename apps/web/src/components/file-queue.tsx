'use client';

import type { FormatInfo } from '@conv.cat/engine';

import type { ConversionJob } from '@/lib/use-converter';
import { FileRow } from './file-row';

interface FileQueueProps {
  jobs: ConversionJob[];
  formats: FormatInfo[];
  onChangeFrom: (id: string, format: FormatInfo) => void;
  onChangeTo: (id: string, format: FormatInfo) => void;
  onConvert: (id: string) => void;
  onCancel: (id: string) => void;
  onRemove: (id: string) => void;
  onConvertAll: () => void;
}

export function FileQueue({
  jobs,
  formats,
  onChangeFrom,
  onChangeTo,
  onConvert,
  onCancel,
  onRemove,
  onConvertAll,
}: FileQueueProps) {
  if (jobs.length === 0) return null;

  const convertibleCount = jobs.filter((job) => job.status === 'ready' || job.status === 'error').length;

  return (
    <section className="panel converter-panel mt-4" aria-label="Files to convert">
      <div className="mb-3 flex items-center justify-between gap-3">
        <h2 className="text-sm font-semibold text-base-content">
          {jobs.length} {jobs.length === 1 ? 'file' : 'files'}
        </h2>
        {convertibleCount > 1 && (
          <button type="button" className="btn-primary" onClick={onConvertAll}>
            Convert all ({convertibleCount})
          </button>
        )}
      </div>

      <ul className="converter-file-list">
        {jobs.map((job) => (
          <FileRow
            key={job.id}
            job={job}
            formats={formats}
            onChangeFrom={(format) => onChangeFrom(job.id, format)}
            onChangeTo={(format) => onChangeTo(job.id, format)}
            onConvert={() => onConvert(job.id)}
            onCancel={() => onCancel(job.id)}
            onRemove={() => onRemove(job.id)}
          />
        ))}
      </ul>
    </section>
  );
}
