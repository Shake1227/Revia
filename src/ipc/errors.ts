import type { AppError, ErrorCode } from "./types";

export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as AppError).code === "string" &&
    typeof (value as AppError).message === "string"
  );
}

export function toAppError(value: unknown): AppError {
  if (isAppError(value)) return { ...value, detail: value.detail ?? null };
  if (value instanceof Error) return { code: "unknown", message: value.message, detail: value.stack ?? value.message };
  if (typeof value === "string") return { code: "unknown", message: value, detail: value };
  return { code: "unknown", message: "Unexpected error", detail: safeStringify(value) };
}

export function appError(code: ErrorCode, message: string, detail: string | null = null): AppError {
  return { code, message, detail };
}

function safeStringify(value: unknown): string | null {
  try {
    return JSON.stringify(value);
  } catch {
    return null;
  }
}
