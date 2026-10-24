/** Independent, time-based damped spring. No frames-per-second assumptions. */
export interface LyricSpring { position: number; velocity: number }

export function advanceLyricSpring(state: LyricSpring, elapsedMs: number): LyricSpring {
  // Exact solution of x'' + 2ζωx' + ω²x = 0. A very small overshoot keeps
  // the landing soft without making text bounce enough to hinder reading.
  const omega = 13;
  const damping = .84;
  const decay = damping * omega;
  const frequency = omega * Math.sqrt(1 - damping * damping);
  const time = Math.max(0, elapsedMs) / 1000;
  const a = state.position;
  const b = (state.velocity + decay * a) / frequency;
  const sin = Math.sin(frequency * time), cos = Math.cos(frequency * time);
  const envelope = Math.exp(-decay * time);
  const position = envelope * (a * cos + b * sin);
  const velocity = envelope * (-decay * (a * cos + b * sin) + frequency * (-a * sin + b * cos));
  return Math.abs(position) < .08 && Math.abs(velocity) < .8
    ? { position: 0, velocity: 0 } : { position, velocity };
}

/** The active line leads; incoming lines follow in a short, bounded cascade. */
export function lyricStagger(index: number, active: number): number {
  return index <= active ? 0 : Math.min(140, (index - active) * 38);
}

export function shouldAnimateLyricChange(previous: number, current: number, timeStep: number): boolean {
  // Scrubbing/backward seeks must not send the entire song flying past the reader.
  return previous >= 0 && current === previous + 1 && timeStep >= 0 && timeStep < 250;
}
