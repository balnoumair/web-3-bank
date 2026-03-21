import { formatUnits, parseUnits } from 'viem';
import { SYNCUSD_DECIMALS } from '~/config/contracts';

/**
 * Format a raw balance (bigint or string in wei) to a display string.
 * Uses 6 decimals (SyncUSD/USDC standard).
 */
export function formatBalance(
  raw: bigint | string,
  decimals: number = SYNCUSD_DECIMALS,
): string {
  const value = typeof raw === 'string' ? BigInt(raw) : raw;
  return formatUnits(value, decimals);
}

/**
 * Format a balance as USD currency string.
 */
export function formatUsd(
  raw: bigint | string,
  decimals: number = SYNCUSD_DECIMALS,
): string {
  const value = Number(formatBalance(raw, decimals));
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(value);
}

/**
 * Parse a human-readable amount string to raw units (bigint).
 */
export function parseAmount(
  amount: string,
  decimals: number = SYNCUSD_DECIMALS,
): bigint {
  return parseUnits(amount, decimals);
}

/**
 * Truncate an address for display: 0x1234...abcd
 */
export function truncateAddress(address: string, chars: number = 4): string {
  if (address.length <= chars * 2 + 2) return address;
  return `${address.slice(0, chars + 2)}...${address.slice(-chars)}`;
}

/**
 * Format a timestamp to a relative or absolute date string.
 */
export function formatDate(timestamp: string | number): string {
  const date = new Date(typeof timestamp === 'string' ? Number(timestamp) * 1000 : timestamp);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffMins < 1) return 'Just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;

  return date.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: date.getFullYear() !== now.getFullYear() ? 'numeric' : undefined,
  });
}
