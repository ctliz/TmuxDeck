export function sanitizeNameFrontend(name: string): string {
  return name
    .trim()
    .replace(/[^A-Za-z0-9_-]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function reorderIds(order: string[], sourceId: string, targetId: string): string[] {
  const next = [...order];
  const fromIndex = next.indexOf(sourceId);
  const toIndex = next.indexOf(targetId);
  if (fromIndex === -1 || toIndex === -1 || fromIndex === toIndex) return next;
  next.splice(fromIndex, 1);
  next.splice(toIndex, 0, sourceId);
  return next;
}
