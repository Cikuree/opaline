// Tri des packs de l'accueil : fonction pure (testée par `node opaline/tests/packs.test.mjs`).
export const AVAIL_FOLD = 4; // au-delà, « À installer » est replié

/** `mine` : packs installés (ordre du catalogue) ; `avail` : packs à installer, non pris en charge en dernier. Filtre par nom. */
export function splitPacks(packs, filter = '') {
  const f = filter.toLowerCase(), ok = (p) => p.name.toLowerCase().includes(f);
  const mine = packs.filter((p) => p.installed && ok(p));
  const avail = packs.filter((p) => !p.installed && ok(p)).sort((a, b) => Number(!!b.supported) - Number(!!a.supported)); // tri stable
  return { mine, avail };
}
