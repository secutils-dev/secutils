import { ensureBundleBuilt } from './bundles.ts';
import { discoverBundles } from './sources.ts';

for (const { name } of discoverBundles()) {
    ensureBundleBuilt(name);
    console.log(`tools-bundles: ${name} ready`);
}
