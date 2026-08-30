export function isAndroidClient() {
  return typeof navigator !== "undefined" && /Android/i.test(navigator.userAgent);
}
