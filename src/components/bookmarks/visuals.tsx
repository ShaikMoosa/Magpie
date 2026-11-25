import clsx from "clsx";
import { useState } from "react";
import { previewSrc } from "../../lib/previews";
import { darken, domainColor, domainLetter, lighten } from "../../lib/colors";

export function Favicon({ domain, size = 20, className, file }: { domain: string; size?: number; className?: string; file?: string | null }) {
  const src = previewSrc(file);
  const [broken, setBroken] = useState(false);
  if (src && !broken) {
    return (
      <img
        src={src}
        alt=""
        aria-hidden
        draggable={false}
        onError={() => setBroken(true)}
        className={clsx("shrink-0 rounded-[5px] bg-canvas object-contain", className)}
        style={{ width: size, height: size }}
      />
    );
  }
  return (
    <span
      aria-hidden
      className={clsx("grid shrink-0 place-items-center rounded-[5px] font-bold text-white", className)}
      style={{ width: size, height: size, background: domainColor(domain), fontSize: Math.round(size * 0.55) }}
    >
      {domainLetter(domain)}
    </span>
  );
}

/** Domain-based placeholder preview (until real Open Graph images arrive). */
export function Thumbnail({ color, className, file }: { color: string; className?: string; file?: string | null }) {
  const src = previewSrc(file);
  const [broken, setBroken] = useState(false);
  if (src && !broken) {
    return (
      <div className={clsx("relative shrink-0 overflow-hidden rounded-[10px] border border-line bg-muted", className)}>
        <img src={src} alt="" aria-hidden draggable={false} onError={() => setBroken(true)} className="size-full object-cover" />
      </div>
    );
  }
  return (
    <div
      aria-hidden
      className={clsx("relative shrink-0 overflow-hidden rounded-[10px]", className)}
      style={{ background: `linear-gradient(135deg, ${lighten(color, 0.85)}, ${lighten(color, 0.25)})` }}
    >
      <div className="absolute top-[18%] left-[14%] h-[70%] w-[77%] rounded-md bg-white/90 p-[7%] shadow-[0_4px_10px_rgb(26_0_77/0.15)]">
        <div className="h-[9%] w-[45%] rounded-sm bg-ink/90" />
        <div className="mt-[6%] h-[6%] w-[80%] rounded-sm bg-ink/30" />
        <div className="mt-[3%] h-[6%] w-[62%] rounded-sm bg-ink/30" />
        <div className="mt-[8%] flex gap-[6%]">
          <div className="h-[22px] flex-1 rounded-sm bg-ink/15" />
          <div className="h-[22px] flex-1 rounded-sm bg-ink/15" />
        </div>
      </div>
    </div>
  );
}

/** 3D folder illustration used on the dashboard (Figma "Folder Card"). */
export function FolderArt({ color, letters, count, lifted }: { color: string; letters: { letter: string; color: string }[]; count: number; lifted?: boolean }) {
  const back = darken(color, 0.18);
  const top = lighten(color, 0.28);
  const lift = lifted ? -6 : 0;
  const id = color.replace("#", "");
  return (
    <div className="relative h-[134px] w-[156px]" aria-hidden>
      <svg className="absolute top-[6px] left-0" width="156" height="112" viewBox="0 0 156 112">
        <defs>
          <linearGradient id={`b${id}`} x1="0" x2="0" y1="0" y2="1">
            <stop offset="0" stopColor={lighten(back, 0.12)} />
            <stop offset="1" stopColor={back} />
          </linearGradient>
        </defs>
        <path fill={`url(#b${id})`} d="M0 14a10 10 0 0 1 10-10h38c4 0 6.5 1.5 9 4l4 4H146a10 10 0 0 1 10 10v78a12 12 0 0 1-12 12H12A12 12 0 0 1 0 100Z" />
      </svg>
      {[
        { left: 18, top: 10, rot: 8 },
        { left: 70, top: 2, rot: -5 },
      ].map((p, i) => (
        <div
          key={i}
          className="absolute h-[68px] w-[58px] rounded-[7px] border border-[#E6E7EC] bg-[#FCFCFD] p-[9px] shadow-[0_2px_6px_rgb(0_0_0/0.12)] transition-transform duration-200"
          style={{ left: p.left, top: p.top, transform: `translateY(${lift}px) rotate(${p.rot}deg)` }}
        >
          <div className="h-1 w-[30px] rounded-sm bg-ink-3/30" />
          <div className="mt-1 h-1 w-10 rounded-sm bg-ink-3/15" />
          <div className="mt-[3px] h-1 w-[34px] rounded-sm bg-ink-3/15" />
        </div>
      ))}
      {letters[0] && (
        <div
          className="absolute top-5 left-[110px] grid size-[38px] place-items-center rounded-[10px] border-[2.5px] border-white text-[17px] font-bold text-white shadow-[0_3px_8px_rgb(0_0_0/0.18)] transition-transform duration-200"
          style={{ background: letters[0].color, transform: `translateY(${lift}px) rotate(-10deg)` }}
        >
          {letters[0].letter}
        </div>
      )}
      <svg className="absolute top-[46px] left-0" width="156" height="86" viewBox="0 0 156 86" style={{ filter: `drop-shadow(0 12px 16px ${color}55)` }}>
        <defs>
          <linearGradient id={`f${id}`} x1="0" x2="0" y1="0" y2="1">
            <stop offset="0" stopColor={top} />
            <stop offset="1" stopColor={color} />
          </linearGradient>
        </defs>
        <path
          fill={`url(#f${id})`}
          stroke="white"
          strokeOpacity="0.45"
          d="M0 22a12 12 0 0 1 12-12h50c4 0 6-1 9-4l2-2c3-3 5-4 9-4h62a12 12 0 0 1 12 12v62a12 12 0 0 1-12 12H12A12 12 0 0 1 0 74Z"
        />
        <rect x="20" y="66" width="116" height="1" fill="white" fillOpacity="0.32" />
        <rect x="20" y="72" width="116" height="1" fill="white" fillOpacity="0.32" />
      </svg>
      <div className="absolute top-[66px] left-3 flex h-6 w-[132px] items-center justify-between">
        <div className="flex">
          {letters.slice(0, 3).map((l, i) => (
            <span
              key={i}
              className="-ml-1.5 grid size-[22px] place-items-center rounded-full text-[10px] font-semibold text-white ring-[1.5px] ring-white first:ml-0"
              style={{ background: l.color }}
            >
              {l.letter}
            </span>
          ))}
        </div>
        <span className="rounded-full border border-white/40 bg-white/25 px-2 py-0.5 text-[11px] font-semibold text-white tabular-nums">{count.toLocaleString()}</span>
      </div>
    </div>
  );
}
