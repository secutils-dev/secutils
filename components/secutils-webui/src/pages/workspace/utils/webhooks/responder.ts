import type { EntityTag } from '../../../../model';

export interface SecretsAccess {
  type: 'none' | 'all' | 'selected';
  secrets?: string[];
}

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
    secrets?: SecretsAccess;
  };
  tags?: EntityTag[];
  createdAt: number;
  updatedAt: number;
}
