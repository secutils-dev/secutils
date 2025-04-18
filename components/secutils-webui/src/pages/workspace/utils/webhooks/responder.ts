export interface Responder {
  id: string;
  name: string;
  location: {
    pathType: '=' | '^';
    path: string;
    subdomainPrefix?: string;
  };
  method: string;
  enabled: boolean;
  settings: {
    requestsToTrack: number;
    statusCode: number;
    headers?: Array<[string, string]>;
    body?: string;
    script?: string;
  };
  createdAt: number;
  updatedAt: number;
}
