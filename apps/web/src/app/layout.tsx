import type { Metadata, Viewport } from 'next';

import { SiteFooter } from '@/components/site-footer';
import { SiteHeader } from '@/components/site-header';
import { THEME_INIT_SCRIPT } from '@/lib/theme';

import './globals.css';

export const metadata: Metadata = {
  title: 'conv.cat — File conversion, purr-fected.',
  description:
    'Convert files entirely on your device. No upload, no account, no server in between — and the source code to prove it.',
};

export const viewport: Viewport = {
  themeColor: [
    { media: '(prefers-color-scheme: dark)', color: '#232733' },
    { media: '(prefers-color-scheme: light)', color: '#faf7f0' },
  ],
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    // suppressHydrationWarning: the inline script below sets `data-theme` before React
    // hydrates, deliberately, to avoid a flash of the wrong theme (see lib/theme.ts). That makes
    // the server-rendered and first-client-rendered `<html>` attributes legitimately differ —
    // this tells React that's expected, instead of logging a spurious warning every load.
    <html lang="en" suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_INIT_SCRIPT }} />
      </head>
      <body className="flex min-h-screen flex-col">
        <a href="#main-content" className="skip-link">
          Skip to content
        </a>
        <SiteHeader />
        {children}
        <SiteFooter />
      </body>
    </html>
  );
}
