import type { Lyric } from './types';

export function normalizeLyrics(lines: Lyric[]): Lyric[] {
  return lines.filter(line => Number.isFinite(line.time_ms) && line.time_ms >= 0)
    .map(line => ({ ...line, time_ms: Math.floor(line.time_ms), text: line.text.trim() || '♪' }))
    .sort((a, b) => a.time_ms - b.time_ms);
}

export function activeLyricIndex(lines: Lyric[], time: number): number {
  let lo = 0, hi = lines.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].time_ms <= time) lo = mid + 1;
    else hi = mid;
  }
  return lo - 1;
}

/** Backend snapshots remain authoritative; only the display clock is interpolated. */
export function displayPosition(position: number, receivedAt: number, now: number, playing: boolean, duration: number): number {
  const value = Math.max(0, position + (playing ? Math.max(0, now - receivedAt) : 0));
  return Math.floor(duration > 0 ? Math.min(duration, value) : value);
}
