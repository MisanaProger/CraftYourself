// Minimal line icons (24×24, currentColor).
const P = { fill: "none", stroke: "currentColor", strokeWidth: 1.8, strokeLinecap: "round" as const, strokeLinejoin: "round" as const };

export const Icon = {
  home: () => (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden>
      <path {...P} d="M4 11l8-7 8 7v8a1 1 0 0 1-1 1h-4v-6h-6v6H5a1 1 0 0 1-1-1z" />
    </svg>
  ),
  test: () => (
    <svg viewBox="0 0 24 24" width="28" height="28" aria-hidden>
      <path {...P} d="M9 3h6M10 3v6L4.5 18.5A1.6 1.6 0 0 0 5.9 21h12.2a1.6 1.6 0 0 0 1.4-2.5L14 9V3" />
      <path {...P} d="M7.5 14h9" />
    </svg>
  ),
  git: () => (
    <svg viewBox="0 0 24 24" width="28" height="28" aria-hidden>
      <circle {...P} cx="6" cy="6" r="2.2" />
      <circle {...P} cx="6" cy="18" r="2.2" />
      <circle {...P} cx="18" cy="12" r="2.2" />
      <path {...P} d="M6 8.2v7.6M8 6.5c5 0 8 1.5 9.2 3.7" />
    </svg>
  ),
  bug: () => (
    <svg viewBox="0 0 24 24" width="28" height="28" aria-hidden>
      <rect {...P} x="7" y="7" width="10" height="13" rx="5" />
      <path {...P} d="M12 11v9M7 12H3M21 12h-4M7 17l-3 2M17 17l3 2M7 8L4 6M17 8l3-2M9 7a3 3 0 0 1 6 0" />
    </svg>
  ),
  map: () => (
    <svg viewBox="0 0 24 24" width="28" height="28" aria-hidden>
      <path {...P} d="M3 6l6-2 6 2 6-2v14l-6 2-6-2-6 2zM9 4v14M15 6v14" />
    </svg>
  ),
  book: () => (
    <svg viewBox="0 0 24 24" width="28" height="28" aria-hidden>
      <path {...P} d="M3 6c3-1.5 6-1.5 9 0 3-1.5 6-1.5 9 0v12c-3-1.5-6-1.5-9 0-3-1.5-6-1.5-9 0zM12 6v12" />
    </svg>
  ),
  tree: () => (
    <svg viewBox="0 0 24 24" width="28" height="28" aria-hidden>
      <circle {...P} cx="12" cy="5" r="2.2" />
      <circle {...P} cx="6" cy="13" r="2.2" />
      <circle {...P} cx="18" cy="13" r="2.2" />
      <circle {...P} cx="4" cy="20" r="1.6" />
      <circle {...P} cx="9" cy="20" r="1.6" />
      <path {...P} d="M10.5 6.5l-3 4.8M13.5 6.5l3 4.8M5.3 15l-.8 3.4M6.9 15l1.4 3.5" />
    </svg>
  ),
  issue: () => (
    <svg viewBox="0 0 24 24" width="28" height="28" aria-hidden>
      <circle {...P} cx="12" cy="12" r="9" />
      <path {...P} d="M12 7v6M12 16.5v.5" />
    </svg>
  ),
  play: () => (
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden>
      <path {...P} d="M7 5l12 7-12 7z" />
    </svg>
  ),
  check: () => (
    <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden>
      <path {...P} strokeWidth={2.4} d="M5 12l5 5 9-10" />
    </svg>
  ),
  chat: () => (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden>
      <path {...P} d="M4 12a8 7 0 1 1 3.5 5.8L4 19l1.2-3.3A6.7 6.7 0 0 1 4 12z" />
    </svg>
  ),
  tasks: () => (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden>
      <rect {...P} x="4" y="4" width="16" height="16" rx="3" />
      <path {...P} d="M8 12l3 3 5-6" />
    </svg>
  ),
  projects: () => (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden>
      <path {...P} d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
    </svg>
  ),
  learning: () => (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden>
      <path {...P} d="M3 6c3-1.5 6-1.5 9 0 3-1.5 6-1.5 9 0v12c-3-1.5-6-1.5-9 0-3-1.5-6-1.5-9 0zM12 6v12" />
    </svg>
  ),
  skills: () => (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden>
      <circle {...P} cx="12" cy="5" r="2.2" />
      <circle {...P} cx="5" cy="18" r="2.2" />
      <circle {...P} cx="19" cy="18" r="2.2" />
      <path {...P} d="M11 7l-5 9M13 7l5 9M7.2 18h9.6" />
    </svg>
  ),
  adi: () => (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden>
      <path {...P} d="M4 20V14M9 20V10M14 20v-6M19 20V6" />
    </svg>
  ),
  settings: () => (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden>
      <circle {...P} cx="12" cy="12" r="3" />
      <path {...P} d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1" />
    </svg>
  ),
  back: () => (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden>
      <path {...P} d="M19 12H5M11 6l-6 6 6 6" />
    </svg>
  ),
  send: () => (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden>
      <path {...P} d="M4 12l16-8-6 16-2.5-6.5z" />
    </svg>
  ),
  clip: () => (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden>
      <path {...P} d="M20 11l-8 8a5 5 0 0 1-7-7l8-8a3.3 3.3 0 0 1 4.7 4.7l-8 8a1.7 1.7 0 0 1-2.4-2.4l7.3-7.3" />
    </svg>
  ),
  up: () => (
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden>
      <path {...P} d="M12 19V5M6 11l6-6 6 6" />
    </svg>
  ),
  chevrons: () => (
    <svg viewBox="0 0 24 24" width="28" height="28" aria-hidden>
      <path {...P} strokeWidth={2.4} d="M6 13l6-6 6 6M6 19l6-6 6 6" />
    </svg>
  ),
  copy: () => (
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden>
      <rect {...P} x="8" y="8" width="12" height="12" rx="2" />
      <path {...P} d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" />
    </svg>
  ),
  branch: () => (
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden>
      <circle {...P} cx="6" cy="5" r="2" />
      <circle {...P} cx="6" cy="19" r="2" />
      <circle {...P} cx="18" cy="8" r="2" />
      <path {...P} d="M6 7v10M18 10c0 4-6 3-12 7" />
    </svg>
  ),
};
