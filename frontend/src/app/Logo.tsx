/** Craft Yourself mark: an open octagonal "C" crossed by an orange "Y". */
export function LogoMark({ size = 36 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden focusable="false" className="logo-mark">
      <path d="M43 15H23L13 25v14l10 10h12" fill="none" stroke="var(--logo-c)" strokeWidth="9" strokeLinejoin="miter" />
      <path d="M27 27l9 12" stroke="var(--accent)" strokeWidth="8" strokeLinecap="square" />
      <path d="M55 9L33 56" stroke="var(--accent)" strokeWidth="9" strokeLinecap="square" />
    </svg>
  );
}

export function Logo() {
  return (
    <div className="logo">
      <LogoMark />
      <div>
        <div className="logo-name">
          Craft
          <br />
          Yourself
        </div>
      </div>
    </div>
  );
}
