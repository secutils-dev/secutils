import type { CertificateAttributes } from './certificate_attributes';
import type { EntityTag } from '../../../../model';

export interface CertificateTemplate {
  id: string;
  name: string;
  attributes: CertificateAttributes;
  tags?: EntityTag[];
  createdAt: number;
  updatedAt: number;
}
