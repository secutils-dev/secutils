import { createHash } from 'node:crypto';

import type { Page, Response } from 'playwright-core';

interface WebPageResources {
  scripts: WebPageResource[];
  styles: WebPageResource[];
}

/**
 * Describes external or inline resource.
 */
interface WebPageResource {
  /**
   * Resource type, either 'script' or 'stylesheet'.
   */
  type: 'script' | 'stylesheet';

  /**
   * The URL resource is loaded from.
   */
  url?: string;

  /**
   * Optional diff status, if the resource is compared with a previous version.
   */
  diff?: WebPageResourceDiffStatus;

  /**
   * Resource content descriptor (size and digest), if available.
   */
  content: WebPageResourceContent;
}

/**
 * Describes the diff status of a web page resource.
 */
enum WebPageResourceDiffStatus {
  Added = 'added',
  Removed = 'removed',
  Modified = 'modified',
}

/**
 * Describes external or inline resource.
 */
interface WebPageResourceRaw {
  /**
   * Resource type, either 'script' or 'stylesheet'.
   */
  type: 'script' | 'stylesheet';

  /**
   * The URL resource is loaded from.
   */
  url?: string;

  /**
   * Resource content descriptor (size and digest), if available.
   */
  content: string;
}

/**
 * Describes resource content.
 */
interface WebPageResourceContent {
  /**
   * Resource content data.
   */
  data: WebPageResourceContentData;

  /**
   * Size of the inline resource content, if available, in bytes.
   */
  size: number;
}

/**
 * Describes resource content data, it can either be the raw content data or a hash such as Trend Micro Locality
 * Sensitive Hash or simple SHA-1.
 */
type WebPageResourceContentData = { raw: string } | { tlsh: string } | { sha1: string };

/**
 * Describes a table view of web page resources.
 */
interface WebPageResourcesTable {
  '@secutils.data.view': 'table';
  columns: Array<{ id: string; label: string; sortable?: boolean }>;
  rows: Array<Record<string, string | { value: string; color?: string } | undefined>>;
  source: WebPageResources;
}

const externalScripts: Array<WebPageResourceRaw> = [];
const externalStyles: Array<WebPageResourceRaw> = [];

const onPageResponse = async (response: Response) => {
  const status = response.status();
  // Ignore responses with non-success status codes.
  if (status < 200 || status >= 300) {
    return;
  }

  const resourceType = response.request().resourceType();
  if (resourceType !== 'script' && resourceType !== 'stylesheet') {
    return;
  }

  const url = response.url();
  try {
    (resourceType === 'script' ? externalScripts : externalStyles).push({
      type: resourceType,
      url,
      content: await response.text(),
    });
  } catch (e) {
    console.error(`Failed to load body of the resource: ${url}`, e);
    return;
  }
};

function createResourceContentData(data: string, maxSize: number): WebPageResourceContentData {
  if (data.length <= maxSize) {
    return { raw: data };
  }

  const hasher = new Tlsh();
  hasher.finale(data);
  try {
    return { tlsh: `T1${hasher.hash().toString()}` };
  } catch (err) {
    console.error(`Failed to calculate TLS hash for resource(size: ${data.length})`, err);
    return { sha1: createHash('sha1').update(data).digest('hex') };
  }
}

const TLSH_CONTENT_SIZE_THRESHOLD_PERCENT = 10.0;
const TLSH_DISTANCE_THRESHOLD = 200;

function diffResources(from: WebPageResource[], to: WebPageResource[]): WebPageResource[] {
  const mapFrom = toResourcesMap(from);
  const mapTo = toResourcesMap(to);
  const diff: WebPageResource[] = [];

  function update(pair: [WebPageResource?, WebPageResource?]) {
    const [toRes, fromRes] = pair;
    if (toRes && fromRes) {
      const eq = JSON.stringify(toRes.content) === JSON.stringify(fromRes.content);
      if (!eq) {
        toRes.diff = WebPageResourceDiffStatus.Modified;
      }
      diff.push(toRes);
    } else if (toRes) {
      toRes.diff = WebPageResourceDiffStatus.Added;
      diff.push(toRes);
    } else if (fromRes) {
      fromRes.diff = WebPageResourceDiffStatus.Removed;
      diff.push(fromRes);
    }
  }

  for (const [key, listTo] of mapTo.resources) {
    const listFrom = mapFrom.resources.get(key) || [];
    if (listFrom.length > 0) {
      zipLongest(listTo, listFrom).forEach(update);
      mapFrom.resources.delete(key);
    } else {
      const first = listTo[0];
      let similarKey: string | undefined;
      if (first && !first.url && 'tlsh' in first.content.data) {
        const hTo = first.content.data.tlsh;
        const sizeTo = first.content.size;
        let best: { hash: string; dist: number } | null = null;
        for (const [hFrom, sizeFrom] of mapFrom.similarityHashes) {
          const absDiff = Math.abs(sizeTo - sizeFrom);
          const avg = (sizeTo + sizeFrom) / 2;
          if ((absDiff / avg) * 100 > TLSH_CONTENT_SIZE_THRESHOLD_PERCENT) {
            if (sizeFrom > sizeTo) {
              break;
            } else {
              continue;
            }
          }
          const d = Tlsh.parse(hFrom).diff(Tlsh.parse(hTo));
          if (d < TLSH_DISTANCE_THRESHOLD && (!best || d < best.dist)) {
            best = { hash: hFrom, dist: d };
          }
        }
        if (best) {
          similarKey = best.hash;
        }
      }
      if (similarKey) {
        const fromList = mapFrom.resources.get(similarKey) || [];
        zipLongest(listTo, fromList).forEach(update);
        mapFrom.resources.delete(similarKey);
      } else {
        listTo.forEach((resource) => {
          resource.diff = WebPageResourceDiffStatus.Added;
          diff.push(resource);
        });
      }
    }
  }

  for (const remaining of mapFrom.resources.values()) {
    remaining.forEach((r) => {
      r.diff = WebPageResourceDiffStatus.Removed;
      diff.push(r);
    });
  }

  return diff;
}

function toResourcesMap(resources: WebPageResource[]) {
  const resourcesMap = new Map<string, WebPageResource[]>();
  const similarityHashes: Array<[string, number]> = [];
  for (const resource of resources) {
    let resourceKey: string;
    if (resource.url) {
      resourceKey = resource.url;
    } else {
      const content = resource.content.data;
      if ('tlsh' in content) {
        similarityHashes.push([content.tlsh, resource.content.size]);
        resourceKey = content.tlsh;
      } else if ('sha1' in content) {
        resourceKey = content.sha1;
      } else if (content.raw) {
        resourceKey = content.raw;
      } else {
        console.error('Resource missing key', resource);
        continue;
      }
    }

    const keyedResources = resourcesMap.get(resourceKey) || [];
    keyedResources.push(resource);
    resourcesMap.set(resourceKey, keyedResources);
  }
  similarityHashes.sort((a, b) => a[1] - b[1]);
  return { resources: resourcesMap, similarityHashes: similarityHashes };
}

function zipLongest<A, B>(a: A[], b: B[]): Array<[A?, B?]> {
  const len = Math.max(a.length, b.length);
  const res: Array<[A?, B?]> = [];
  for (let i = 0; i < len; i++) {
    res.push([a[i], b[i]]);
  }
  return res;
}

export const resources = {
  startTracking: (page: Page) => {
    externalScripts.length = 0;
    externalStyles.length = 0;

    // Listen for responses to capture external resources.
    page.on('response', onPageResponse);
  },
  stopTracking: async (page: Page, maxSizeBytes = 1024): Promise<WebPageResources> => {
    page.off('response', onPageResponse);

    const { scripts: inlineScripts, styles: inlineStyles } = await page.evaluate(async () => {
      async function parseURL(url: string): Promise<{ url: string; data: string }> {
        if (url.startsWith('data:')) {
          // For `data:` URLs we should replace the actual content the digest later.
          return { url: `${url.split(',')[0]},`, data: url };
        }

        if (url.startsWith('blob:')) {
          // For `blob:` URLs we should fetch the actual content and replace object reference with the digest later.
          return {
            url: 'blob:',
            // [BUG] There is a bug in Node.js 20.4.0 that doesn't properly handle `await response.text()` in tests.
            data: await fetch(url)
              .then((res) => res.body?.getReader().read())
              .then((res) => new TextDecoder().decode(res?.value)),
          };
        }

        return { url, data: '' };
      }

      function isResourceValid(resource: WebPageResourceRaw) {
        return !!(resource.url || resource.content.trim());
      }

      const inlineScripts: Array<WebPageResourceRaw> = [];
      for (const el of Array.from(document.querySelectorAll('script'))) {
        // We treat script content as a concatenation of `onload` handler and its inner content. For our purposes it
        // doesn't matter if the script is loaded from an external source or is inline. If later we figure out that
        // script content was also loaded from the external source (e.g. when `script` element has both `src` and
        // `innerHTML`) we'll re-calculate its digest and size.
        const { url, data } = await parseURL(el.src.trim());
        const resource = { type: 'script' as const, url: url ? url : undefined, content: data };
        const scriptContent = (el.onload?.toString().trim() ?? '') + el.innerHTML.trim() + data;
        if (scriptContent) {
          resource.content = await new Blob([scriptContent]).text();
        }

        if (isResourceValid(resource)) {
          inlineScripts.push(resource);
        }
      }

      const inlineStylesheets: Array<WebPageResourceRaw> = [];
      for (const el of Array.from(document.querySelectorAll('link[rel=stylesheet]'))) {
        const { url, data } = await parseURL((el as HTMLLinkElement).href.trim());
        const resource = {
          type: 'stylesheet' as const,
          url: url ? url : undefined,
          content: data ? await new Blob([data]).text() : data,
        };

        if (isResourceValid(resource)) {
          inlineStylesheets.push(resource);
        }
      }

      for (const el of Array.from(document.querySelectorAll('style'))) {
        const contentBlob = new Blob([el.innerHTML]);
        if (contentBlob.size > 0) {
          inlineStylesheets.push({ type: 'stylesheet', content: await contentBlob.text() });
        }
      }

      return { scripts: inlineScripts, styles: inlineStylesheets };
    });

    const result: WebPageResources = { scripts: [], styles: [] };
    for (const [inlineResources, externalResources, combinedResources] of [
      [inlineScripts, externalScripts, result.scripts],
      [inlineStyles, externalStyles, result.styles],
    ] as Array<[WebPageResourceRaw[], WebPageResourceRaw[], WebPageResource[]]>) {
      for (const inlineResource of inlineResources) {
        // If the inline resource has a URL, we should check if it was fetched externally.
        const externalResourceIndex = inlineResource.url
          ? externalResources.findIndex((res) => res.url === inlineResource.url)
          : -1;
        // Combine inline and external resources, if URL matches.
        let rawData: string;
        if (externalResourceIndex >= 0) {
          const [externalResource] = externalResources.splice(
            externalResourceIndex,
            1,
          ) as unknown as WebPageResourceRaw[];
          rawData = externalResource.content + inlineResource.content;
        } else {
          rawData = inlineResource.content;
        }

        combinedResources.push({
          type: inlineResource.type,
          url: inlineResource.url,
          content: { size: rawData.length, data: createResourceContentData(rawData, maxSizeBytes) },
        });
      }

      for (const externalResource of externalResources) {
        combinedResources.push({
          type: externalResource.type,
          url: externalResource.url,
          content: {
            size: externalResource.content.length,
            data: createResourceContentData(externalResource.content, maxSizeBytes),
          },
        });
      }
    }

    return result;
  },
  setDiffStatus: (previousResources: WebPageResources, currentResources: WebPageResources) => {
    return {
      scripts: diffResources(previousResources.scripts, currentResources.scripts),
      styles: diffResources(previousResources.styles, currentResources.styles),
    };
  },
  formatAsTable: (resources: WebPageResources): WebPageResourcesTable => {
    return {
      '@secutils.data.view': 'table',
      columns: [
        { id: 'source', label: 'Source', sortable: true },
        { id: 'diff', label: 'Diff', sortable: true },
        { id: 'type', label: 'Type', sortable: true },
        { id: 'size', label: 'Size', sortable: true },
      ],
      rows: [...resources.scripts, ...resources.styles].map((resource) => {
        const diff =
          resource.diff === WebPageResourceDiffStatus.Added
            ? { value: 'Added', color: '#6dccb1' }
            : resource.diff === WebPageResourceDiffStatus.Removed
              ? { value: 'Removed', color: '#ff7e62' }
              : resource.diff === WebPageResourceDiffStatus.Modified
                ? { value: 'Changed', color: '#79aad9' }
                : undefined;
        const source = resource.url ?? '(inline)';
        return {
          source: diff ? { value: source, color: diff.color } : source,
          diff,
          type: resource.type === 'script' ? 'Script' : 'Stylesheet',
          size: resource.content.size.toString(),
        };
      }),
      source: resources,
    };
  },
};

/*
 * TLSH is provided for use under two licenses: Apache OR BSD.
 * Users may opt to use either license depending on the license
 * restrictions of the systems with which they plan to integrate
 * the TLSH code.
 */

/* ==============
 * Apache License
 * ==============
 * Copyright 2013 Trend Micro Incorporated
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/* ===========
 * BSD License
 * ===========
 * Copyright (c) 2013, Trend Micro Incorporated
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without modification,
 * are permitted provided that the following conditions are met:
 *
 * 1. Redistributions of source code must retain the above copyright notice, this
 *    list of conditions and the following disclaimer.
 *
 * 2. Redistributions in binary form must reproduce the above copyright notice,
 *    this list of conditions and the following disclaimer in the documentation
 *    and/or other materials provided with the distribution.

 * 3. Neither the name of the copyright holder nor the names of its contributors
 *    may be used to endorse or promote products derived from this software without
 *    specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
 * WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.
 * IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
 * INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
 * BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
 * LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE
 * OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
 * OF THE POSSIBILITY OF SUCH DAMAGE.
 */

/*
 * Port of C++ implementation tlsh to javascript.
 *
 * Construct Tlsh object with methods:
 *   update
 *   finale
 *   reset
 *   hash
 *   diff
 *   parse
 *
 * See tlsh.html for example use.
 */

const TLSH_CHECKSUM_LEN = 1;
const MIN_DATA_LENGTH = 50;
const SLIDING_WND_SIZE = 5;
const RNG_SIZE = SLIDING_WND_SIZE;
const BUCKETS = 256;
// 128 * 2 bits = 32 bytes
const CODE_SIZE = 32;
// 2 + 1 + 32 bytes = 70 hexidecimal chars
const TLSH_STRING_LEN = 70;
const RANGE_LVALUE = 256;
const RANGE_QRATIO = 16;
const EFF_BUCKETS = 128;

const V_TABLE = new Uint8Array([
  1, 87, 49, 12, 176, 178, 102, 166, 121, 193, 6, 84, 249, 230, 44, 163, 14, 197, 213, 181, 161, 85, 218, 80, 64, 239,
  24, 226, 236, 142, 38, 200, 110, 177, 104, 103, 141, 253, 255, 50, 77, 101, 81, 18, 45, 96, 31, 222, 25, 107, 190, 70,
  86, 237, 240, 34, 72, 242, 20, 214, 244, 227, 149, 235, 97, 234, 57, 22, 60, 250, 82, 175, 208, 5, 127, 199, 111, 62,
  135, 248, 174, 169, 211, 58, 66, 154, 106, 195, 245, 171, 17, 187, 182, 179, 0, 243, 132, 56, 148, 75, 128, 133, 158,
  100, 130, 126, 91, 13, 153, 246, 216, 219, 119, 68, 223, 78, 83, 88, 201, 99, 122, 11, 92, 32, 136, 114, 52, 10, 138,
  30, 48, 183, 156, 35, 61, 26, 143, 74, 251, 94, 129, 162, 63, 152, 170, 7, 115, 167, 241, 206, 3, 150, 55, 59, 151,
  220, 90, 53, 23, 131, 125, 173, 15, 238, 79, 95, 89, 16, 105, 137, 225, 224, 217, 160, 37, 123, 118, 73, 2, 157, 46,
  116, 9, 145, 134, 228, 207, 212, 202, 215, 69, 229, 27, 188, 67, 124, 168, 252, 42, 4, 29, 108, 21, 247, 19, 205, 39,
  203, 233, 40, 186, 147, 198, 192, 155, 33, 164, 191, 98, 204, 165, 180, 117, 76, 140, 36, 210, 172, 41, 54, 159, 8,
  185, 232, 113, 196, 231, 47, 146, 120, 51, 65, 28, 144, 254, 221, 93, 189, 194, 139, 112, 43, 71, 109, 184, 209,
]);

function b_mapping(salt: number, i: number, j: number, k: number) {
  let h = 0;

  h = V_TABLE[h ^ salt];
  h = V_TABLE[h ^ i];
  h = V_TABLE[h ^ j];
  h = V_TABLE[h ^ k];

  return h;
}

const LOG_1_5 = 0.4054651;
const LOG_1_3 = 0.26236426;
const LOG_1_1 = 0.09531018;

function l_capturing(len: number) {
  let i;
  if (len <= 656) {
    i = Math.floor(Math.log(len) / LOG_1_5);
  } else if (len <= 3199) {
    i = Math.floor(Math.log(len) / LOG_1_3 - 8.72777);
  } else {
    i = Math.floor(Math.log(len) / LOG_1_1 - 62.5472);
  }

  return i & 0xff;
}

// Use  generateTable() from TLSH.java implementation
const bit_pairs_diff_table = (() => {
  const arraySize = 256;
  const result = Array(arraySize)
    .fill(0)
    .map(() => new Uint8Array(arraySize));
  for (let i = 0; i < arraySize; i++) {
    for (let j = 0; j < arraySize; j++) {
      let x = i,
        y = j,
        d,
        diff = 0;
      d = Math.abs((x % 4) - (y % 4));
      diff += d == 3 ? 6 : d;
      x = Math.floor(x / 4);
      y = Math.floor(y / 4);

      d = Math.abs((x % 4) - (y % 4));
      diff += d == 3 ? 6 : d;
      x = Math.floor(x / 4);
      y = Math.floor(y / 4);

      d = Math.abs((x % 4) - (y % 4));
      diff += d == 3 ? 6 : d;
      x = Math.floor(x / 4);
      y = Math.floor(y / 4);

      d = Math.abs((x % 4) - (y % 4));
      diff += d == 3 ? 6 : d;
      result[i][j] = diff;
    }
  }
  return result;
})();

function h_distance(len: number, x: Uint8Array, y: Uint8Array) {
  let diff = 0;
  for (let i = 0; i < len; i++) {
    diff += bit_pairs_diff_table[x[i]][y[i]];
  }
  return diff;
}

function getQLo(Q: number) {
  return Q & 0x0f;
}

function getQHi(Q: number) {
  return (Q & 0xf0) >> 4;
}

function setQLo(Q: number, x: number) {
  return (Q & 0xf0) | (x & 0x0f);
}

function setQHi(Q: number, x: number) {
  return (Q & 0x0f) | ((x & 0x0f) << 4);
}

function partition(buf: Buffer, left: number, right: number) {
  if (left === right) {
    return left;
  }

  if (left + 1 == right) {
    if (buf.bucket_copy[left] > buf.bucket_copy[right]) {
      SWAP_UINT(buf, left, right);
    }
    return left;
  }

  let ret = left;
  const pivot = (left + right) >> 1;

  const val = buf.bucket_copy[pivot];

  buf.bucket_copy[pivot] = buf.bucket_copy[right];
  buf.bucket_copy[right] = val;

  for (let i = left; i < right; i++) {
    if (buf.bucket_copy[i] < val) {
      SWAP_UINT(buf, ret, i);
      ret++;
    }
  }
  buf.bucket_copy[right] = buf.bucket_copy[ret];
  buf.bucket_copy[ret] = val;

  return ret;
}

function swap_byte(i: number) {
  return (((i & 0xf0) >> 4) & 0x0f) | (((i & 0x0f) << 4) & 0xf0);
}

function to_hex(data: Uint8Array, len: number) {
  // Use TLSH.java implementation for to_hex
  let s = '';
  for (let i = 0; i < len; i++) {
    if (data[i] < 16) {
      s = s.concat('0');
    }
    s = s.concat(data[i].toString(16).toUpperCase());
  }

  return s;
}

function from_hex(str: string) {
  // Use TLSH.java implementation for from_hex
  const ret = new Uint8Array(str.length / 2); // unsigned char array}
  for (let i = 0; i < str.length; i += 2) {
    ret[i / 2] = parseInt(str.substring(i, i + 2), 16);
  }
  return ret;
}

function mod_diff(x: number, y: number, R: number) {
  let dl: number;
  let dr: number;
  if (y > x) {
    dl = y - x;
    dr = x + R - y;
  } else {
    dl = x - y;
    dr = y + R - x;
  }
  return dl > dr ? dr : dl;
}

function SWAP_UINT(buf: Buffer, left: number, right: number) {
  const int_tmp = buf.bucket_copy[left];
  buf.bucket_copy[left] = buf.bucket_copy[right];
  buf.bucket_copy[right] = int_tmp;
}

function RNG_IDX(i: number) {
  return (i + RNG_SIZE) % RNG_SIZE;
}

interface Quartiles {
  q1: number;
  q2: number;
  q3: number;
}

interface Buffer {
  bucket_copy: Uint32Array;
}

interface TempHash {
  checksum: Uint8Array;
  Lvalue: number;
  Q: number;
  tmp_code: Uint8Array;
}

///////////////////////////////////////////////////////////////////////////////////
// Definition of tlsh object
export class Tlsh {
  checksum = new Uint8Array(TLSH_CHECKSUM_LEN); // unsigned char array
  slide_window = new Uint8Array(SLIDING_WND_SIZE);
  a_bucket = new Uint32Array(BUCKETS); // unsigned int array
  data_len = 0;
  tmp_code = new Uint8Array(CODE_SIZE);
  Lvalue = 0;
  Q = 0;
  lsh_code = '';
  lsh_code_valid = false;

  update(str: string) {
    const data = new Uint8Array(Buffer.from(str, 'utf-8'));

    let j = this.data_len % RNG_SIZE;
    let fed_len = this.data_len;

    for (let i = 0; i < data.length; i++, fed_len++, j = RNG_IDX(j + 1)) {
      this.slide_window[j] = data[i];

      if (fed_len >= 4) {
        //only calculate when input >= 5 bytes
        const j_1 = RNG_IDX(j - 1);
        const j_2 = RNG_IDX(j - 2);
        const j_3 = RNG_IDX(j - 3);
        const j_4 = RNG_IDX(j - 4);

        for (let k = 0; k < TLSH_CHECKSUM_LEN; k++) {
          if (k == 0) {
            this.checksum[k] = b_mapping(0, this.slide_window[j], this.slide_window[j_1], this.checksum[k]);
          } else {
            // use calculated 1 byte checksums to expand the total checksum to 3 bytes
            this.checksum[k] = b_mapping(
              this.checksum[k - 1],
              this.slide_window[j],
              this.slide_window[j_1],
              this.checksum[k],
            );
          }
        }

        let r = b_mapping(2, this.slide_window[j], this.slide_window[j_1], this.slide_window[j_2]);
        r = b_mapping(2, this.slide_window[j], this.slide_window[j_1], this.slide_window[j_2]);
        r = b_mapping(2, this.slide_window[j], this.slide_window[j_1], this.slide_window[j_2]);

        this.a_bucket[r]++;
        r = b_mapping(3, this.slide_window[j], this.slide_window[j_1], this.slide_window[j_3]);
        this.a_bucket[r]++;
        r = b_mapping(5, this.slide_window[j], this.slide_window[j_2], this.slide_window[j_3]);
        this.a_bucket[r]++;
        r = b_mapping(7, this.slide_window[j], this.slide_window[j_2], this.slide_window[j_4]);
        this.a_bucket[r]++;
        r = b_mapping(11, this.slide_window[j], this.slide_window[j_1], this.slide_window[j_4]);
        this.a_bucket[r]++;
        r = b_mapping(13, this.slide_window[j], this.slide_window[j_3], this.slide_window[j_4]);
        this.a_bucket[r]++;
      }
    }
    this.data_len += data.length;
  }

  // final is a reserved word
  finale(str: string) {
    if (typeof str !== 'undefined') {
      this.update(str);
    }

    if (this.data_len < MIN_DATA_LENGTH) {
      throw new Error(`ERROR: length too small - ${this.data_len}`);
    }

    const quartiles: Quartiles = { q1: 0, q2: 0, q3: 0 };
    this.find_quartile(quartiles);

    // buckets must be more than 50% non-zero
    let nonzero = 0;
    for (let i = 0; i < CODE_SIZE; i++) {
      for (let j = 0; j < 4; j++) {
        if (this.a_bucket[4 * i + j] > 0) {
          nonzero++;
        }
      }
    }
    if (nonzero <= (4 * CODE_SIZE) / 2) {
      throw new Error(`ERROR: not enough variation in input - ${nonzero} < ${(4 * CODE_SIZE) / 2}`);
    }

    for (let i = 0; i < CODE_SIZE; i++) {
      let h = 0;
      for (let j = 0; j < 4; j++) {
        const k = this.a_bucket[4 * i + j];
        if (quartiles.q3 < k) {
          h += 3 << (j * 2); // leave the optimization j*2 = j<<1 or j*2 = j+j for compiler
        } else if (quartiles.q2 < k) {
          h += 2 << (j * 2);
        } else if (quartiles.q1 < k) {
          h += 1 << (j * 2);
        }
      }
      this.tmp_code[i] = h;
    }

    this.Lvalue = l_capturing(this.data_len);
    this.Q = setQLo(this.Q, ((quartiles.q1 * 100) / quartiles.q3) % 16);
    this.Q = setQHi(this.Q, ((quartiles.q2 * 100) / quartiles.q3) % 16);
    this.lsh_code_valid = true;
  }

  find_quartile(quartiles: Quartiles) {
    const buf: Buffer = {
      bucket_copy: new Uint32Array(EFF_BUCKETS),
    };

    const short_cut_left = new Uint32Array(EFF_BUCKETS);
    const short_cut_right = new Uint32Array(EFF_BUCKETS);
    let spl = 0;
    let spr = 0;
    const p1 = EFF_BUCKETS / 4 - 1;
    const p2 = EFF_BUCKETS / 2 - 1;
    const p3 = EFF_BUCKETS - EFF_BUCKETS / 4 - 1;
    const end = EFF_BUCKETS - 1;

    for (let i = 0; i <= end; i++) {
      buf.bucket_copy[i] = this.a_bucket[i];
    }

    for (let l = 0, r = end; ; ) {
      const ret = partition(buf, l, r);
      if (ret > p2) {
        r = ret - 1;
        short_cut_right[spr] = ret;
        spr++;
      } else if (ret < p2) {
        l = ret + 1;
        short_cut_left[spl] = ret;
        spl++;
      } else {
        quartiles.q2 = buf.bucket_copy[p2];
        break;
      }
    }

    short_cut_left[spl] = p2 - 1;
    short_cut_right[spr] = p2 + 1;

    for (let i = 0, l = 0; i <= spl; i++) {
      let r = short_cut_left[i];
      if (r > p1) {
        for (;;) {
          const ret = partition(buf, l, r);
          if (ret > p1) {
            r = ret - 1;
          } else if (ret < p1) {
            l = ret + 1;
          } else {
            quartiles.q1 = buf.bucket_copy[p1];
            break;
          }
        }
        break;
      } else if (r < p1) {
        l = r;
      } else {
        quartiles.q1 = buf.bucket_copy[p1];
        break;
      }
    }

    for (let i = 0, r = end; i <= spr; i++) {
      let l = short_cut_right[i];
      if (l < p3) {
        for (;;) {
          const ret = partition(buf, l, r);
          if (ret > p3) {
            r = ret - 1;
          } else if (ret < p3) {
            l = ret + 1;
          } else {
            quartiles.q3 = buf.bucket_copy[p3];
            break;
          }
        }
        break;
      } else if (l > p3) {
        r = l;
      } else {
        quartiles.q3 = buf.bucket_copy[p3];
        break;
      }
    }
  }

  hash() {
    if (!this.lsh_code_valid) {
      throw new Error('ERROR IN PROCESSING');
    }

    const tmp: TempHash = {
      checksum: new Uint8Array(TLSH_CHECKSUM_LEN),
      Lvalue: 0,
      Q: 0,
      tmp_code: new Uint8Array(CODE_SIZE),
    };

    for (let k = 0; k < TLSH_CHECKSUM_LEN; k++) {
      tmp.checksum[k] = swap_byte(this.checksum[k]);
    }
    tmp.Lvalue = swap_byte(this.Lvalue);
    tmp.Q = swap_byte(this.Q);

    for (let i = 0; i < CODE_SIZE; i++) {
      tmp.tmp_code[i] = this.tmp_code[CODE_SIZE - 1 - i];
    }

    this.lsh_code = to_hex(tmp.checksum, TLSH_CHECKSUM_LEN);

    const tmpArray = new Uint8Array(1);
    tmpArray[0] = tmp.Lvalue;
    this.lsh_code = this.lsh_code.concat(to_hex(tmpArray, 1));

    tmpArray[0] = tmp.Q;
    this.lsh_code = this.lsh_code.concat(to_hex(tmpArray, 1));
    this.lsh_code = this.lsh_code.concat(to_hex(tmp.tmp_code, CODE_SIZE));
    return this.lsh_code;
  }

  diff(other: Tlsh, len_diff = true) {
    if (this == other) {
      return 0;
    }

    let diff = 0;
    if (len_diff) {
      const lDiff = mod_diff(this.Lvalue, other.Lvalue, RANGE_LVALUE);
      if (lDiff == 0) {
        diff = 0;
      } else if (lDiff == 1) {
        diff = 1;
      } else {
        diff += lDiff * 12;
      }
    }

    const q1diff = mod_diff(getQLo(this.Q), getQLo(other.Q), RANGE_QRATIO);
    if (q1diff <= 1) {
      diff += q1diff;
    } else {
      diff += (q1diff - 1) * 12;
    }

    const q2diff = mod_diff(getQHi(this.Q), getQHi(other.Q), RANGE_QRATIO);
    if (q2diff <= 1) {
      diff += q2diff;
    } else {
      diff += (q2diff - 1) * 12;
    }

    for (let k = 0; k < TLSH_CHECKSUM_LEN; k++) {
      if (this.checksum[k] != other.checksum[k]) {
        diff++;
        break;
      }
    }

    diff += h_distance(CODE_SIZE, this.tmp_code, other.tmp_code);

    return diff;
  }

  static parse(str: string) {
    const hashStr = str.length === TLSH_STRING_LEN + 2 && str.startsWith('T1') ? str.slice(2) : str;
    if (hashStr.length != TLSH_STRING_LEN) {
      throw new Error(`Tlsh.parse() - string has wrong length (${hashStr.length} !=  ${TLSH_STRING_LEN})`);
    }

    for (const char of hashStr) {
      if (!((char >= '0' && char <= '9') || (char >= 'A' && char <= 'F') || (char >= 'a' && char <= 'f'))) {
        throw new Error(`Tlsh.parse() - string has invalid (non-hex) characters: ${char}`);
      }
    }

    const tmp = from_hex(hashStr);
    const tlsh = new Tlsh();

    // Order of assignment is based on order of fields in lsh_bin, also note that TLSH_CHECKSUM_LEN is 1.
    let i = 0;
    tlsh.checksum[i] = swap_byte(tmp[i++]);
    tlsh.Lvalue = swap_byte(tmp[i++]);
    tlsh.Q = swap_byte(tmp[i++]);

    for (let j = 0; j < CODE_SIZE; j++) {
      tlsh.tmp_code[j] = tmp[i + CODE_SIZE - 1 - j];
    }
    tlsh.lsh_code_valid = true;

    return tlsh;
  }
}
