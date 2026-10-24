import type { Snapshot } from './types';
export type Update = Omit<Snapshot, 'queue'> & { queue?: Snapshot['queue'] };

/** Subscribe first, buffer during snapshot acquisition, then reject stale revisions. */
export class StateSync {
  private pending: Update[] = [];
  private ready = false;
  constructor(private state: Snapshot, private apply: (state: Snapshot) => void) {}
  event(update: Update) {
    if (!this.ready) { this.pending.push(update); return; }
    this.merge(update);
  }
  snapshot(snapshot: Snapshot) {
    this.state = snapshot;
    this.ready = true;
    this.apply(this.state);
    for (const update of this.pending.sort((a, b) => a.revision - b.revision)) this.merge(update);
    this.pending = [];
  }
  private merge(update: Update) {
    if (update.revision <= this.state.revision) return;
    this.state = { ...this.state, ...update, queue: update.queue ?? this.state.queue };
    this.apply(this.state);
  }
}
