import { describe, expect, it } from 'vitest';
import { StateSync } from './sync';
import type { Snapshot } from './types';
const state: Snapshot = { revision: 10, queue_revision: 1, queue: [], current_id: null,
  playback: 'playing', position_ms: 1000, duration_ms: 10000, volume: .8, repeat: 'off', shuffle: false, continuous: { kind: 'none' }, error: null };
describe('core snapshot and event synchronization', () => {
  it('buffers events during initialization and ignores late old snapshots', () => {
    let latest = state;
    const sync = new StateSync(state, value => { latest = value; });
    sync.event({ ...state, revision: 12, position_ms: 2000 });
    sync.event({ ...state, revision: 9, position_ms: 0 });
    sync.snapshot(state);
    expect(latest.position_ms).toBe(2000);
    sync.event({ ...state, revision: 11, position_ms: 1500 });
    expect(latest.position_ms).toBe(2000);
  });
  it('preserves the queue when progress events omit it', () => {
    let latest = state;
    const sync = new StateSync(state, value => { latest = value; });
    sync.snapshot(state);
    const { queue: _, ...event } = state;
    sync.event({ ...event, revision: 11, position_ms: 1250 });
    expect(latest.queue).toBe(state.queue);
    expect(latest.position_ms).toBe(1250);
  });
});
