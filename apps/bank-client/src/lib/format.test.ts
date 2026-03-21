import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { formatBalance, formatUsd, parseAmount, truncateAddress, formatDate } from './format';

describe('formatBalance', () => {
  it('formats 1 USDC (6 decimals)', () => {
    expect(formatBalance(1_000_000n)).toBe('1');
  });

  it('formats fractional amounts', () => {
    expect(formatBalance(1_500_000n)).toBe('1.5');
    expect(formatBalance(500_000n)).toBe('0.5');
  });

  it('formats zero', () => {
    expect(formatBalance(0n)).toBe('0');
  });

  it('accepts string input', () => {
    expect(formatBalance('1000000')).toBe('1');
    expect(formatBalance('0')).toBe('0');
  });

  it('respects custom decimals', () => {
    expect(formatBalance(1_000_000_000_000_000_000n, 18)).toBe('1');
  });

  it('formats large amounts', () => {
    expect(formatBalance(1_000_000_000_000n)).toBe('1000000');
  });
});

describe('formatUsd', () => {
  it('formats whole dollar amounts', () => {
    expect(formatUsd(1_000_000n)).toBe('$1.00');
    expect(formatUsd(100_000_000n)).toBe('$100.00');
  });

  it('formats cents', () => {
    expect(formatUsd(1_500_000n)).toBe('$1.50');
    expect(formatUsd(1_010_000n)).toBe('$1.01');
  });

  it('formats zero', () => {
    expect(formatUsd(0n)).toBe('$0.00');
  });

  it('rounds to 2 decimal places', () => {
    // 1.005 USDC rounded
    expect(formatUsd(1_005_000n)).toBe('$1.01');
  });
});

describe('parseAmount', () => {
  it('parses whole numbers', () => {
    expect(parseAmount('1')).toBe(1_000_000n);
    expect(parseAmount('100')).toBe(100_000_000n);
  });

  it('parses decimals', () => {
    expect(parseAmount('1.5')).toBe(1_500_000n);
    expect(parseAmount('0.5')).toBe(500_000n);
    expect(parseAmount('0.000001')).toBe(1n);
  });

  it('parses zero', () => {
    expect(parseAmount('0')).toBe(0n);
  });

  it('respects custom decimals', () => {
    expect(parseAmount('1', 18)).toBe(1_000_000_000_000_000_000n);
  });

  it('round-trips with formatBalance', () => {
    const raw = parseAmount('42.5');
    expect(formatBalance(raw)).toBe('42.5');
  });
});

describe('truncateAddress', () => {
  const fullAddress = '0x1234567890abcdef1234567890abcdef12345678';

  it('truncates to default 4 chars on each side', () => {
    expect(truncateAddress(fullAddress)).toBe('0x1234...5678');
  });

  it('respects custom chars param', () => {
    expect(truncateAddress(fullAddress, 6)).toBe('0x123456...345678');
  });

  it('returns short addresses unchanged', () => {
    expect(truncateAddress('0x1234')).toBe('0x1234');
    expect(truncateAddress('0xabcd')).toBe('0xabcd');
  });

  it('handles exact boundary length without truncating', () => {
    // chars=4: address must be > 4*2+2=10 chars to get truncated
    const short = '0x12345678'; // length=10, not > 10
    expect(truncateAddress(short)).toBe(short);
  });
});

describe('formatDate', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2024-06-15T12:00:00.000Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns "Just now" for timestamps within the last minute', () => {
    const nowSeconds = Math.floor(Date.now() / 1000);
    expect(formatDate(String(nowSeconds))).toBe('Just now');
    expect(formatDate(String(nowSeconds - 30))).toBe('Just now');
  });

  it('returns minutes ago for timestamps within the last hour', () => {
    const fiveMinutesAgo = Math.floor(Date.now() / 1000) - 5 * 60;
    expect(formatDate(String(fiveMinutesAgo))).toBe('5m ago');

    const fiftyNineMinutesAgo = Math.floor(Date.now() / 1000) - 59 * 60;
    expect(formatDate(String(fiftyNineMinutesAgo))).toBe('59m ago');
  });

  it('returns hours ago for timestamps within the last 24 hours', () => {
    const threeHoursAgo = Math.floor(Date.now() / 1000) - 3 * 3600;
    expect(formatDate(String(threeHoursAgo))).toBe('3h ago');

    const twentyThreeHoursAgo = Math.floor(Date.now() / 1000) - 23 * 3600;
    expect(formatDate(String(twentyThreeHoursAgo))).toBe('23h ago');
  });

  it('returns days ago for timestamps within the last 7 days', () => {
    const twoDaysAgo = Math.floor(Date.now() / 1000) - 2 * 86400;
    expect(formatDate(String(twoDaysAgo))).toBe('2d ago');

    const sixDaysAgo = Math.floor(Date.now() / 1000) - 6 * 86400;
    expect(formatDate(String(sixDaysAgo))).toBe('6d ago');
  });

  it('returns formatted date for timestamps older than 7 days (same year)', () => {
    const tenDaysAgo = Math.floor(Date.now() / 1000) - 10 * 86400;
    const result = formatDate(String(tenDaysAgo));
    expect(result).toMatch(/^Jun \d+$/); // e.g. "Jun 5"
  });

  it('returns formatted date with year for timestamps in a different year', () => {
    const lastYear = Math.floor(new Date('2023-01-01T00:00:00.000Z').getTime() / 1000);
    const result = formatDate(String(lastYear));
    expect(result).toMatch(/^Jan \d+, 2023$/); // e.g. "Jan 1, 2023"
  });

  it('accepts numeric millisecond timestamps', () => {
    const nowMs = Date.now() - 10 * 60 * 1000; // 10 minutes ago in ms
    expect(formatDate(nowMs)).toBe('10m ago');
  });
});
