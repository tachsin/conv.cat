'use client';

import { CommandPalette } from '@/components/command-palette';
import { DropZone } from '@/components/drop-zone';
import { FileQueue } from '@/components/file-queue';
import { PrivacyBadge } from '@/components/privacy-badge';
import { useConverter } from '@/lib/use-converter';
import { useTheme } from '@/lib/use-theme';

export default function HomePage() {
  const {
    jobs,
    formats,
    engineLoading,
    engineLoadError,
    addFiles,
    setJobFormat,
    removeJob,
    convertJob,
    cancelJob,
    convertAll,
    announcement,
  } = useConverter();
  const [theme, setTheme] = useTheme();
  const convertibleCount = jobs.filter((job) => job.status === 'ready' || job.status === 'error').length;

  return (
    <main id="main-content" className="converter-stack converter-stack--narrow mx-auto px-4 py-8 sm:px-6 sm:py-10">
      <div className="animate-fade-up flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="page-eyebrow">File conversion, purr-fected.</p>
          <h1 className="mt-1 text-2xl font-semibold tracking-tight text-base-content sm:text-3xl">
            Convert a file, on this device, right now.
          </h1>
          <p className="mt-2 text-sm leading-relaxed text-base-content/60">
            Drop in one or more files, pick where they should end up, and download the result.
            Nothing leaves your browser.
          </p>
        </div>
        <CommandPalette
          jobs={jobs}
          convertibleCount={convertibleCount}
          isDark={theme === 'convcat-dark'}
          onAddFiles={addFiles}
          onConvertAll={() => void convertAll()}
          onConvertJob={(id) => void convertJob(id)}
          onRemoveJob={removeJob}
          onToggleTheme={() => setTheme(theme === 'convcat-dark' ? 'convcat' : 'convcat-dark')}
        />
      </div>

      <PrivacyBadge />

      {engineLoadError ? (
        <div role="alert" className="glass-card rounded-xl border border-error/30 p-4 text-sm text-error">
          The conversion engine failed to load: {engineLoadError}. Try reloading the page.
        </div>
      ) : (
        <DropZone
          onFiles={(files) => addFiles(files)}
          disabled={engineLoading}
          disabledReason="Loading the conversion engine…"
        />
      )}

      <FileQueue
        jobs={jobs}
        formats={formats}
        onChangeFrom={(id, format) => setJobFormat(id, 'from', format)}
        onChangeTo={(id, format) => setJobFormat(id, 'to', format)}
        onConvert={(id) => void convertJob(id)}
        onCancel={cancelJob}
        onRemove={removeJob}
        onConvertAll={() => void convertAll()}
      />

      {/* Screen-reader-only status channel: announces file additions, conversion start/finish,
          and errors without visually duplicating the per-row status text already on screen (see
          FileRow's `converter-file-status` paragraph, which is the sighted equivalent). */}
      <p aria-live="polite" role="status" className="sr-only">
        {announcement}
      </p>
    </main>
  );
}
