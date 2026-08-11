'use client';

// The one piece of state management this app needs: a queue of files, each carrying its own
// detected/chosen formats, progress and result. All conversion work is delegated to
// `@conv.cat/engine` — this hook never parses a byte of file content itself (see this package's
// README: "UI only... must never contain conversion logic").

import { useCallback, useEffect, useRef, useState } from 'react';
import { ConvertError, getEngine } from '@conv.cat/engine';
import type { ConvEngine, EngineBackendKind, FormatInfo } from '@conv.cat/engine';

import { guessFormat, readableFormats, writableFormatsInCategory } from './formats';

export type JobStatus = 'detecting' | 'needs-format' | 'ready' | 'converting' | 'done' | 'error';

export interface ConversionJob {
  readonly id: string;
  readonly file: File;
  readonly status: JobStatus;
  readonly from?: FormatInfo;
  readonly to?: FormatInfo;
  /** Fraction in `0..=1` — see `ConvertOptions.onProgress`'s own contract: not every converter
   * reports this evenly, so this is a "best current guess", not a precise ETA. */
  readonly progress: number;
  readonly resultUrl?: string;
  readonly resultBytes?: number;
  readonly errorMessage?: string;
}

interface EngineState {
  engine: ConvEngine | null;
  backend: EngineBackendKind | null;
  formats: FormatInfo[];
  memoryCeilingBytes: number | null;
  loading: boolean;
  loadError: string | null;
}

let nextId = 0;
function newJobId(): string {
  nextId += 1;
  return `job-${nextId}-${Date.now().toString(36)}`;
}

export function useConverter() {
  const [engineState, setEngineState] = useState<EngineState>({
    engine: null,
    backend: null,
    formats: [],
    memoryCeilingBytes: null,
    loading: true,
    loadError: null,
  });
  const [jobs, setJobs] = useState<ConversionJob[]>([]);
  const [announcement, setAnnouncement] = useState('');
  const abortControllers = useRef(new Map<string, AbortController>());
  const objectUrls = useRef(new Set<string>());

  // Load the engine once. `getEngine()` itself is idempotent (module-level singleton) but this
  // still guards against setting state after unmount in dev's StrictMode double-invoke.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const engine = await getEngine();
        const [formats, memoryCeilingBytes] = await Promise.all([
          engine.listFormats(),
          engine.getMemoryCeilingBytes(),
        ]);
        if (cancelled) return;
        setEngineState({
          engine,
          backend: engine.backend,
          formats,
          memoryCeilingBytes,
          loading: false,
          loadError: null,
        });
      } catch (error) {
        if (cancelled) return;
        setEngineState((state) => ({
          ...state,
          loading: false,
          loadError: error instanceof Error ? error.message : String(error),
        }));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Revoke every still-live object URL on unmount — each is a `Blob` held alive by the browser
  // until explicitly released.
  useEffect(() => {
    const urls = objectUrls.current;
    return () => {
      for (const url of urls) URL.revokeObjectURL(url);
      urls.clear();
    };
  }, []);

  const patchJob = useCallback((id: string, patch: Partial<ConversionJob>) => {
    setJobs((current) => current.map((job) => (job.id === id ? { ...job, ...patch } : job)));
  }, []);

  const addFiles = useCallback(
    (files: Iterable<File>) => {
      const incoming = Array.from(files);
      if (incoming.length === 0) return;
      const newJobs: ConversionJob[] = incoming.map((file) => {
        const from = guessFormat(file, engineState.formats);
        return {
          id: newJobId(),
          file,
          from,
          to: from,
          status: from ? 'ready' : 'needs-format',
          progress: 0,
        };
      });
      setJobs((current) => [...current, ...newJobs]);
      setAnnouncement(
        incoming.length === 1
          ? `Added ${incoming[0].name}.`
          : `Added ${incoming.length} files.`,
      );
    },
    [engineState.formats],
  );

  const setJobFormat = useCallback(
    (id: string, kind: 'from' | 'to', format: FormatInfo) => {
      setJobs((current) =>
        current.map((job) => {
          if (job.id !== id) return job;
          if (kind === 'from') {
            const to = writableFormatsInCategory(engineState.formats, format.category)[0];
            return { ...job, from: format, to, status: to ? 'ready' : 'needs-format' };
          }
          return { ...job, to: format };
        }),
      );
    },
    [engineState.formats],
  );

  const removeJob = useCallback((id: string) => {
    abortControllers.current.get(id)?.abort();
    abortControllers.current.delete(id);
    setJobs((current) => {
      const job = current.find((candidate) => candidate.id === id);
      if (job?.resultUrl) {
        URL.revokeObjectURL(job.resultUrl);
        objectUrls.current.delete(job.resultUrl);
      }
      return current.filter((candidate) => candidate.id !== id);
    });
  }, []);

  const convertJob = useCallback(
    async (id: string) => {
      const engine = engineState.engine;
      if (!engine) return;
      const job = jobs.find((candidate) => candidate.id === id);
      if (!job?.from || !job.to) return;

      const controller = new AbortController();
      abortControllers.current.set(id, controller);
      patchJob(id, { status: 'converting', progress: 0, errorMessage: undefined });
      setAnnouncement(`Converting ${job.file.name}…`);

      try {
        const input = new Uint8Array(await job.file.arrayBuffer());
        const output = await engine.convert(input, job.from.id, job.to.id, {
          signal: controller.signal,
          onProgress: (fraction) => patchJob(id, { progress: fraction }),
        });
        // `.slice()` (not the raw `output`): guarantees a plain `ArrayBuffer`-backed copy,
        // which is what `BlobPart`'s stricter modern DOM typing requires — `engine.convert()`'s
        // return type is `Uint8Array<ArrayBufferLike>`, permissive enough to also cover a
        // (never actually returned here) `SharedArrayBuffer`-backed view.
        const blob = new Blob([output.slice()], { type: job.to.mime });
        const url = URL.createObjectURL(blob);
        objectUrls.current.add(url);
        patchJob(id, { status: 'done', progress: 1, resultUrl: url, resultBytes: output.byteLength });
        setAnnouncement(`${job.file.name} converted.`);
      } catch (error) {
        const message =
          error instanceof ConvertError
            ? describeConvertError(error)
            : error instanceof Error
              ? error.message
              : String(error);
        patchJob(id, { status: 'error', errorMessage: message });
        setAnnouncement(`${job.file.name} failed to convert: ${message}`);
      } finally {
        abortControllers.current.delete(id);
      }
    },
    [engineState.engine, jobs, patchJob],
  );

  const cancelJob = useCallback((id: string) => {
    abortControllers.current.get(id)?.abort();
  }, []);

  const convertAll = useCallback(async () => {
    const pending = jobs.filter((job) => job.status === 'ready' || job.status === 'error');
    for (const job of pending) {
      // Sequential on purpose: the WASM engine is single-threaded (one shared Worker — see
      // packages/engine/src/wasm/client.ts), so parallel `convert()` calls would only queue up
      // inside it anyway. Sequential await keeps the visible progress legible instead of several
      // bars crawling at once for no real throughput gain.
      await convertJob(job.id);
    }
  }, [jobs, convertJob]);

  return {
    engine: engineState.engine,
    backend: engineState.backend,
    formats: engineState.formats,
    memoryCeilingBytes: engineState.memoryCeilingBytes,
    engineLoading: engineState.loading,
    engineLoadError: engineState.loadError,
    jobs,
    addFiles,
    setJobFormat,
    removeJob,
    convertJob,
    cancelJob,
    convertAll,
    announcement,
    readableFileFormats: readableFormats(engineState.formats),
  };
}

function describeConvertError(error: ConvertError): string {
  switch (error.kind) {
    case 'unsupported_pair':
      return "this app can't convert between those two formats yet";
    case 'malformed_input':
      return "the file doesn't look like valid input for its detected format";
    case 'unsupported_feature':
      return 'this specific case is a documented gap, not implemented yet';
    case 'size_limit_exceeded':
    case 'memory_limit_exceeded':
      return 'the file is too large for this browser to convert';
    case 'cancelled':
      return 'cancelled';
    case 'backend_unavailable':
      return "this browser can't run the conversion engine (Web Workers may be disabled)";
    default:
      return error.message;
  }
}
