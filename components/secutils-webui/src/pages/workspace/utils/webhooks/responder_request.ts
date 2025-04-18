export interface ResponderRequest {
  id: string;
  clientAddress?: string;
  method: string;
  headers?: Array<[string, number[]]>;
  url: string;
  body?: number[];
  createdAt: number;
}
