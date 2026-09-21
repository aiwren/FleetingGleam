/** One motion language for the whole app. Durations in seconds, springs for anything that moves in space. */
export const EASE_OUT = [0.16, 1, 0.3, 1] as const; // expo-out: fast start, long settle
export const DUR = { fast: 0.16, base: 0.32, slow: 0.6, develop: 0.9 } as const;

export const SPRING = {
  glide: { type: "spring", stiffness: 420, damping: 40 }, // layout reflow, selection ring, pills
  soft: { type: "spring", stiffness: 170, damping: 26 },  // entrances that should feel weighty
  view: { type: "spring", stiffness: 230, damping: 30 },  // photo <-> lightbox
  tap: { type: "spring", stiffness: 600, damping: 32 },   // hover / press feedback
} as const;
