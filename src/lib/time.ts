import type { Messages } from "../i18n";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export function relativeTime(at: number | null | undefined, m: Messages, now = Date.now()): string {
  if (at === null || at === undefined) return m.time.never;
  const diff = Math.max(0, now - at);
  if (diff < MINUTE) return m.time.now;
  if (diff < HOUR) return m.time.minutes(Math.floor(diff / MINUTE));
  if (diff < DAY) return m.time.hours(Math.floor(diff / HOUR));
  const days = Math.floor(diff / DAY);
  if (days === 1) return m.time.yesterday;
  if (days < 7) return m.time.days(days);
  if (days < 31) return m.time.weeks(Math.floor(days / 7));
  if (days < 365) return m.time.months(Math.floor(days / 30));
  return m.time.years(Math.floor(days / 365));
}

const formats = {
  dateTime: { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" },
  dateTimeYear: { year: "numeric", month: "short", day: "numeric", hour: "numeric", minute: "2-digit" },
  timeOnly: { hour: "numeric", minute: "2-digit" },
  dayLabel: { month: "long", day: "numeric", weekday: "short" },
  dayLabelYear: { year: "numeric", month: "long", day: "numeric" },
} satisfies Record<string, Intl.DateTimeFormatOptions>;

const formatters = new Map<string, Intl.DateTimeFormat>();

function formatter(m: Messages, name: keyof typeof formats): Intl.DateTimeFormat {
  const key = `${m.locale}:${name}`;
  let found = formatters.get(key);
  if (!found) {
    found = new Intl.DateTimeFormat(m.locale, formats[name]);
    formatters.set(key, found);
  }
  return found;
}

export function formatDateTime(at: number, m: Messages, now = Date.now()): string {
  const sameYear = new Date(at).getFullYear() === new Date(now).getFullYear();
  return formatter(m, sameYear ? "dateTime" : "dateTimeYear").format(at);
}

export function formatTime(at: number, m: Messages): string {
  return formatter(m, "timeOnly").format(at);
}

export function startOfDay(at: number): number {
  const date = new Date(at);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

export function formatDay(at: number, m: Messages, now = Date.now()): string {
  const diff = Math.round((startOfDay(now) - startOfDay(at)) / DAY);
  if (diff === 0) return m.history.today;
  if (diff === 1) return m.history.yesterday;
  const sameYear = new Date(at).getFullYear() === new Date(now).getFullYear();
  return formatter(m, sameYear ? "dayLabel" : "dayLabelYear").format(at);
}
