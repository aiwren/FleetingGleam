/** A size in bytes the way the language writes it ("1.2 MB", "1,2 Mo"), through Intl rather than hand-written units. */
export function fmtBytes(lang: string, b: number): string {
  const shown = b <= 0 ? 0 : Math.max(b, 1e3);
  const u = shown >= 1e9 ? ["gigabyte", 1e9, 1] : shown >= 1e6 ? ["megabyte", 1e6, shown >= 1e7 ? 0 : 1] : ["kilobyte", 1e3, 0];
  return new Intl.NumberFormat(lang, { style: "unit", unit: u[0] as string, maximumFractionDigits: u[2] as number }).format(shown / (u[1] as number));
}

/** 下载速率（"3.2 MB/s"、"420 kB/s"）。 */
export function fmtSpeed(lang: string, bps: number): string {
  return `${fmtBytes(lang, bps)}/s`;
}
