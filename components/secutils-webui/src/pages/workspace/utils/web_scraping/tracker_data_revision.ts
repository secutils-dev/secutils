export interface TrackerDataRevision<D = unknown> {
  id: string;
  data: { original: D; mods?: D[] };
  createdAt: number;
}
