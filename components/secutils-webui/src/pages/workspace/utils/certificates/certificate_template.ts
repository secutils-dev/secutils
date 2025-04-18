import type { CertificateAttributes } from './certificate_attributes';

export interface CertificateTemplate {
  id: string;
  name: string;
  attributes: CertificateAttributes;
  createdAt: number;
  updatedAt: number;
}
