import { useState } from 'react';
import type { Bucket } from '../types';
import { bytes, time } from '../lib';

export default function Chart({ timeline }: { timeline: Bucket[] }) {
  const [hover, setHover] = useState<number | null>(null);
  const max = Math.max(
    1,
    ...timeline.flatMap((b) => [b.upload, b.download, b.localBytes]),
  );
  const width = 880,
    height = 190,
    pad = 8;
  const x = (i: number) =>
    pad + (i / Math.max(1, timeline.length - 1)) * (width - pad * 2);
  const y = (n: number) => height - 16 - (n / max) * (height - 32);
  const points = (key: 'upload' | 'download' | 'localBytes') =>
    timeline.map((b, i) => `${x(i)},${y(b[key])}`).join(' ');
  if (!timeline.length)
    return (
      <div className="chart-empty">
        Traffic will appear here when observations arrive.
      </div>
    );
  const selected = hover === null ? undefined : timeline[hover];
  return (
    <div className="chart-wrap">
      <div className="chart-scale">
        <span>{bytes(max)}</span>
        <span>{bytes(max / 2)}</span>
        <span>0 B</span>
      </div>
      <svg
        className="traffic-chart"
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-label="Traffic volume per five-minute bucket. Download is green, upload is blue, local transfers are amber."
      >
        <defs>
          <linearGradient id="fill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#378c76" stopOpacity=".18" />
            <stop offset="100%" stopColor="#378c76" stopOpacity="0" />
          </linearGradient>
        </defs>
        {[0, 0.5, 1].map((n) => (
          <line
            key={n}
            x1="0"
            x2={width}
            y1={y(max * n)}
            y2={y(max * n)}
            stroke="#e8ece9"
            strokeDasharray="4 5"
          />
        ))}
        <polygon
          points={`${pad},${height} ${points('download')} ${x(timeline.length - 1)},${height}`}
          fill="url(#fill)"
        />
        <polyline
          points={points('download')}
          fill="none"
          stroke="#348b72"
          strokeWidth="2.5"
          strokeLinejoin="round"
        />
        <polyline
          points={points('upload')}
          fill="none"
          stroke="#6284c4"
          strokeWidth="2.5"
          strokeLinejoin="round"
        />
        <polyline
          points={points('localBytes')}
          fill="none"
          stroke="#c1a25a"
          strokeWidth="2"
          strokeLinejoin="round"
        />
        {hover !== null && (
          <line
            x1={x(hover)}
            x2={x(hover)}
            y1="0"
            y2={height}
            stroke="#82918a"
          />
        )}
      </svg>
      <div className="chart-times">
        {[
          timeline[0],
          timeline[Math.floor(timeline.length / 2)],
          timeline[timeline.length - 1],
        ].map((b, i) => (
          <span key={i}>{time(b.timestamp)}</span>
        ))}
      </div>
      <div className="chart-controls" aria-label="Inspect traffic buckets">
        {timeline.map((b, i) => (
          <button
            key={b.timestamp}
            aria-label={`${time(b.timestamp)}: download ${bytes(b.download)}, upload ${bytes(b.upload)}, local ${bytes(b.localBytes)}`}
            onMouseEnter={() => setHover(i)}
            onMouseLeave={() => setHover(null)}
            onFocus={() => setHover(i)}
            onBlur={() => setHover(null)}
          />
        ))}
      </div>
      {selected && (
        <div className="chart-tooltip">
          {time(selected.timestamp)} · ↓ {bytes(selected.download)} · ↑{' '}
          {bytes(selected.upload)} · Local {bytes(selected.localBytes)}
        </div>
      )}
    </div>
  );
}
