// Génère src/licences.json : licences des composants open source embarqués dans CES Compagnon.
// - Rust : `cargo metadata` (toutes les dépendances résolues de l'application, sclog compris).
// - JavaScript : dépendances de PRODUCTION (`npm ls --omit=dev --all`), licence lue dans chaque package.json.
// À relancer quand les dépendances changent :  node scripts/licences.mjs
import { execSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const app = join(dirname(fileURLToPath(import.meta.url)), '..');

// --- Rust
const meta = JSON.parse(execSync('cargo metadata --format-version 1 --filter-platform x86_64-pc-windows-msvc --manifest-path src-tauri/Cargo.toml', {
  cwd: app, maxBuffer: 256 * 1024 * 1024, encoding: 'utf8',
}));
const locaux = new Set(meta.workspace_members.concat(meta.packages.filter((p) => !p.source).map((p) => p.id)));
// Seulement ce qui est réellement compilé pour Windows (le graphe résolu, filtré par plateforme).
const resolus = new Set((meta.resolve?.nodes || []).map((n) => n.id));
const rust = meta.packages
  .filter((p) => !locaux.has(p.id) && resolus.has(p.id))
  .map((p) => ({ nom: p.name, version: p.version, licence: p.license || (p.license_file ? 'voir fichier de licence' : 'non indiquée') }))
  .sort((a, b) => a.nom.localeCompare(b.nom));

// --- JavaScript (production seulement)
const arbre = JSON.parse(execSync('npm ls --omit=dev --all --json', { cwd: app, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }));
const vus = new Map();
const parcourir = (deps) => {
  for (const [nom, d] of Object.entries(deps || {})) {
    const cle = `${nom}@${d.version}`;
    if (!vus.has(cle)) {
      const pj = join(app, 'node_modules', nom, 'package.json');
      let licence = 'non indiquée';
      if (existsSync(pj)) {
        const p = JSON.parse(readFileSync(pj, 'utf8'));
        licence = typeof p.license === 'string' ? p.license : (p.license?.type || (p.licenses || []).map((l) => l.type || l).join(' OR ') || licence);
      }
      vus.set(cle, { nom, version: d.version, licence });
    }
    parcourir(d.dependencies);
  }
};
parcourir(arbre.dependencies);
const js = [...vus.values()].sort((a, b) => a.nom.localeCompare(b.nom));

writeFileSync(join(app, 'src', 'licences.json'), JSON.stringify({ genere: new Date().toISOString().slice(0, 10), rust, js }, null, 1) + '\n');
console.log(`licences.json : ${rust.length} composants Rust, ${js.length} composants JavaScript`);
