import { describe, expect, it } from 'vitest';

import { buildAnchoredCron, detectSchedulePreset, getScheduleMinInterval, parseAnchorParams } from './consts';
import type { AnchorParams } from './consts';

describe('detectSchedulePreset', () => {
  it('detects hourly pattern', () => {
    expect(detectSchedulePreset('0 30 * * * *')).toBe('@hourly');
    expect(detectSchedulePreset('0 0 * * * *')).toBe('@hourly');
    expect(detectSchedulePreset('0 59 * * * *')).toBe('@hourly');
  });

  it('detects daily pattern', () => {
    expect(detectSchedulePreset('0 30 14 * * *')).toBe('@daily');
    expect(detectSchedulePreset('0 0 0 * * *')).toBe('@daily');
    expect(detectSchedulePreset('0 59 23 * * *')).toBe('@daily');
  });

  it('detects weekly pattern', () => {
    expect(detectSchedulePreset('0 55 19 * * 2')).toBe('@weekly');
    expect(detectSchedulePreset('0 0 0 * * 0')).toBe('@weekly');
    expect(detectSchedulePreset('0 30 12 * * 6')).toBe('@weekly');
  });

  it('detects monthly pattern', () => {
    expect(detectSchedulePreset('0 55 19 15 * *')).toBe('@monthly');
    expect(detectSchedulePreset('0 0 0 1 * *')).toBe('@monthly');
    expect(detectSchedulePreset('0 30 12 28 * *')).toBe('@monthly');
  });

  it('returns null for non-matching patterns', () => {
    expect(detectSchedulePreset('0 30 9 * * Mon,Wed')).toBeNull();
    expect(detectSchedulePreset('0 * * * * *')).toBeNull();
    expect(detectSchedulePreset('30 0 * * * *')).toBeNull();
    expect(detectSchedulePreset('0 30 14 15 * 2')).toBeNull();
  });

  it('returns null for wrong number of fields', () => {
    expect(detectSchedulePreset('0 * * * *')).toBeNull();
    expect(detectSchedulePreset('0 0 0 * * * *')).toBeNull();
  });

  it('returns null for non-zero seconds field', () => {
    expect(detectSchedulePreset('5 30 * * * *')).toBeNull();
  });

  it('returns null for non-wildcard month field', () => {
    expect(detectSchedulePreset('0 30 14 * 6 *')).toBeNull();
  });

  it('returns null for non-numeric values in expected-numeric positions', () => {
    expect(detectSchedulePreset('0 */5 * * * *')).toBeNull();
    expect(detectSchedulePreset('0 30 */2 * * *')).toBeNull();
    expect(detectSchedulePreset('0 30 14 * * Mon')).toBeNull();
    expect(detectSchedulePreset('0 30 14 1-15 * *')).toBeNull();
  });

  it('returns null for preset aliases', () => {
    expect(detectSchedulePreset('@hourly')).toBeNull();
    expect(detectSchedulePreset('@daily')).toBeNull();
    expect(detectSchedulePreset('@weekly')).toBeNull();
    expect(detectSchedulePreset('@monthly')).toBeNull();
  });
});

describe('getScheduleMinInterval', () => {
  it('returns correct intervals for preset aliases', () => {
    expect(getScheduleMinInterval('@hourly')).toBe(3600000);
    expect(getScheduleMinInterval('@daily')).toBe(86400000);
    expect(getScheduleMinInterval('@weekly')).toBe(604800000);
    expect(getScheduleMinInterval('@monthly')).toBe(2592000000);
  });

  it('returns correct intervals for anchored cron expressions', () => {
    expect(getScheduleMinInterval('0 30 * * * *')).toBe(3600000);
    expect(getScheduleMinInterval('0 30 14 * * *')).toBe(86400000);
    expect(getScheduleMinInterval('0 55 19 * * 2')).toBe(604800000);
    expect(getScheduleMinInterval('0 55 19 15 * *')).toBe(2592000000);
  });

  it('returns 0 for unrecognized schedules', () => {
    expect(getScheduleMinInterval('0 30 9 * * Mon,Wed')).toBe(0);
    expect(getScheduleMinInterval('custom-string')).toBe(0);
  });
});

describe('buildAnchoredCron', () => {
  const params: AnchorParams = { minute: 30, hour: 9, weekday: 1, dayOfMonth: 15 };

  it('builds hourly cron (only minute)', () => {
    expect(buildAnchoredCron('@hourly', params)).toBe('0 30 * * * *');
  });

  it('builds daily cron (minute + hour)', () => {
    expect(buildAnchoredCron('@daily', params)).toBe('0 30 9 * * *');
  });

  it('builds weekly cron (minute + hour + weekday)', () => {
    expect(buildAnchoredCron('@weekly', params)).toBe('0 30 9 * * 1');
  });

  it('builds monthly cron (minute + hour + day of month)', () => {
    expect(buildAnchoredCron('@monthly', params)).toBe('0 30 9 15 * *');
  });

  it('passes through non-preset values', () => {
    expect(buildAnchoredCron('0 */5 * * * *', params)).toBe('0 */5 * * * *');
    expect(buildAnchoredCron('@@', params)).toBe('@@');
  });
});

describe('parseAnchorParams', () => {
  it('parses hourly anchored cron', () => {
    expect(parseAnchorParams('0 30 * * * *')).toEqual({
      minute: 30,
      hour: 0,
      weekday: 0,
      dayOfMonth: 1,
    });
  });

  it('parses daily anchored cron', () => {
    expect(parseAnchorParams('0 30 14 * * *')).toEqual({
      minute: 30,
      hour: 14,
      weekday: 0,
      dayOfMonth: 1,
    });
  });

  it('parses weekly anchored cron', () => {
    expect(parseAnchorParams('0 55 19 * * 2')).toEqual({
      minute: 55,
      hour: 19,
      weekday: 2,
      dayOfMonth: 1,
    });
  });

  it('parses monthly anchored cron', () => {
    expect(parseAnchorParams('0 30 9 15 * *')).toEqual({
      minute: 30,
      hour: 9,
      weekday: 0,
      dayOfMonth: 15,
    });
  });

  it('returns null for non-preset crons', () => {
    expect(parseAnchorParams('0 30 9 * * Mon,Wed')).toBeNull();
    expect(parseAnchorParams('@weekly')).toBeNull();
    expect(parseAnchorParams('custom')).toBeNull();
  });

  it('roundtrips with buildAnchoredCron', () => {
    const params: AnchorParams = { minute: 45, hour: 16, weekday: 5, dayOfMonth: 22 };
    for (const preset of ['@hourly', '@daily', '@weekly', '@monthly']) {
      const cron = buildAnchoredCron(preset, params);
      const parsed = parseAnchorParams(cron);
      expect(parsed).not.toBeNull();
      expect(buildAnchoredCron(preset, parsed!)).toBe(cron);
    }
  });
});
