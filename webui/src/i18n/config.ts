export const locales = ["en", "de", "fr", "es", "nl"] as const;
export type Locale = (typeof locales)[number];
export const defaultLocale: Locale = "en";
