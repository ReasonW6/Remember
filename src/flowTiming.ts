export function parseSpeedMultiplier(speed: string) {
  const multiplier = Number(speed.replace(/x$/i, ""));
  return Number.isFinite(multiplier) && multiplier > 0 ? multiplier : 1;
}
