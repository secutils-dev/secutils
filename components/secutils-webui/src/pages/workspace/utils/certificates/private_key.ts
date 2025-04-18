import type { PrivateKeyAlgorithm } from './private_key_alg';

// Describes an instance of a private key.
export interface PrivateKey {
  id: string;
  name: string;
  alg: PrivateKeyAlgorithm;
  encrypted: boolean;
  createdAt: number;
  updatedAt: number;
}
