/**
 * Represents the stats of a responder.
 */
export interface ResponderStats {
  responderId: string;
  requestCount: number;
  lastRequestedAt?: number;
}
