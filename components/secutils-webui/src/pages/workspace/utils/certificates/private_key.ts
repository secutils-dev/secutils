import type { PrivateKeyAlgorithm } from './private_key_alg';
import type { EntityTag } from '../../../../model';

// Describes an instance of a private key.
export interface PrivateKey {
  id: string;
  name: string;
  alg: PrivateKeyAlgorithm;
  encrypted: boolean;
  tags?: EntityTag[];
  createdAt: number;
  updatedAt: number;
}
