// Activity feed — persisted to a JSON file so entries survive server restarts.
// Kept dependency-free so the feed/pagination logic is unit-testable in
// isolation from Express, x402, and runtime config.

import { existsSync, readFileSync, writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DATA_DIR = process.env.ACTIVITY_FEED_DIR || join(__dirname, '../../data');
const FEED_FILE = join(DATA_DIR, 'activityFeed.json');

// Capacity of the feed and pagination bounds.
export const ACTIVITY_MAX_ENTRIES = 50;
export const ACTIVITY_DEFAULT_LIMIT = 20;
export const ACTIVITY_MAX_LIMIT = ACTIVITY_MAX_ENTRIES;

function ensureDataDir() {
  if (!existsSync(DATA_DIR)) {
    mkdirSync(DATA_DIR, { recursive: true });
  }
}

function loadFeed() {
  try {
    ensureDataDir();
    if (!existsSync(FEED_FILE)) return [];
    const raw = readFileSync(FEED_FILE, 'utf-8');
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function saveFeed(feed) {
  try {
    ensureDataDir();
    writeFileSync(FEED_FILE, JSON.stringify(feed, null, 2), 'utf-8');
  } catch (err) {
    console.error('[activityFeed] Failed to persist feed:', err.message);
  }
}

export function recordActivity(entry) {
  const feed = loadFeed();
  feed.unshift(entry);
  if (feed.length > ACTIVITY_MAX_ENTRIES) feed.pop();
  saveFeed(feed);
}

export function getActivityFeed() {
  return loadFeed();
}

/**
 * Validate and normalise `limit`/`offset` query params for the activity feed.
 * Missing params fall back to sane defaults; `limit` is clamped to ACTIVITY_MAX_LIMIT.
 * @param {Record<string, unknown>} [query]
 * @returns {{ limit: number, offset: number, errors: string[] }}
 */
export function parseActivityPagination(query = {}) {
  const errors = [];
  let limit = ACTIVITY_DEFAULT_LIMIT;
  let offset = 0;

  if (query.limit !== undefined) {
    const n = Number(query.limit);
    if (!Number.isInteger(n) || n < 1) {
      errors.push('`limit` must be a positive integer');
    } else {
      limit = Math.min(n, ACTIVITY_MAX_LIMIT);
    }
  }

  if (query.offset !== undefined) {
    const n = Number(query.offset);
    if (!Number.isInteger(n) || n < 0) {
      errors.push('`offset` must be a non-negative integer');
    } else {
      offset = n;
    }
  }

  return { limit, offset, errors };
}
