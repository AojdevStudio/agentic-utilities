declare module "@juicesharp/rpiv-i18n" {
  export type LocaleCode = string;
  export const SUPPORTED_LOCALES: readonly string[];
  export function scope(namespace: string): (key: string, fallback: string) => string;
  export function t(namespace: string, key: string, fallback: string): string;
}

declare module "@juicesharp/rpiv-i18n/loader" {
  export function registerLocalesFromDir(namespace: string, packageUrl: string, options?: { label?: string }): void;
}
