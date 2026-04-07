// SPDX-License-Identifier: GPL-3.0-only

const STORAGE_KEY = "snapdog_api_key";

export function getApiKey(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem(STORAGE_KEY);
}

export function setApiKey(key: string) {
  localStorage.setItem(STORAGE_KEY, key);
}

export function clearApiKey() {
  localStorage.removeItem(STORAGE_KEY);
}
