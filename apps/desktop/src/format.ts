/** A size in bytes the way the language writes it ("1.2 MB", "1,2 Mo"), through Intl rather than hand-written units. */
export function fmtBytes(lang: string, b: number): string {
  const u = b >= 1e9 ? ["gigabyte", 1e9, 1] : b >= 1e6 ? ["megabyte", 1e6, b >= 1e7 ? 0 : 1] : ["kilobyte", 1e3, 0];
  return new Intl.NumberFormat(lang, { style: "unit", unit: u[0] as string, maximumFractionDigits: u[2] as number }).format(Math.max(b, 1e3) / (u[1] as number));
}
