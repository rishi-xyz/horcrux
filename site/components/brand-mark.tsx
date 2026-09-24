export function BrandMark({ size = 22 }: { size?: number }) {
  return (
    <svg
      viewBox="0 0 32 32"
      width={size}
      height={size}
      className="text-accent"
      aria-hidden="true"
    >
      <path
        d="M16 4l10 5v7c0 7.4-4.4 11.9-10 12.6C10.4 27.9 6 23.4 6 16V9l10-5z"
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
      />
      <circle cx={16} cy={15} r={2.6} fill="currentColor" />
      <path d="M16 17.6V23" stroke="currentColor" strokeWidth={2} strokeLinecap="round" />
    </svg>
  );
}
