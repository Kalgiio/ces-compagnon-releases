import { defineConfig, transformWithEsbuild } from 'vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import path from 'node:path';
import fs from 'node:fs';
import tailwindcss from 'tailwindcss';
import autoprefixer from 'autoprefixer';
import pkg from './package.json' with { type: 'json' };

const ici = path.dirname(fileURLToPath(import.meta.url));
// Adresse du portail (même règle que le cœur Rust, portail.rs) : production, sauf CES_PORTAIL en développement.
const PORTAIL = (process.env.CES_PORTAIL || 'https://app.ces-sc.fr').replace(/\/$/, '');
const require = createRequire(import.meta.url);
// Le portail (frontend/src) est PARTAGÉ, pas copié : Aurore Design pour l'application, et tout le site pour la
// webview du site embarqué (site.html). `frontend/` reste la source unique.
const portail = path.resolve(ici, '../../frontend/src');
const aurore = path.join(portail, 'design-system');
const norme = (p) => p.replace(/\\/g, '/');
const portailN = norme(portail);

// Éléments du portail remplacés dans l'application (chemin sans extension, relatif à frontend/src).
const SUBSTITUTIONS = {
  'config/env': 'src/site/env.js',
  'components/SharedSidebar': 'src/site/BarreSite.jsx',
  'components/SharedFooter': 'src/site/Vide.jsx',
  'components/CompanionWatch': 'src/site/Vide.jsx',
  'pages/AuthPage': 'src/site/SansSession.jsx',
  'pages/NoAccessPage': 'src/site/SansSession.jsx',
};

// Tailwind du portail (configuration CommonJS) ; ses extensions sont résolues depuis CE projet.
// (évaluée avec le `require` de ce projet : frontend/ n'a pas de node_modules ici)
const chargerTailwind = () => {
  const module = { exports: {} };
  const source = fs.readFileSync(path.resolve(portail, '../tailwind.config.js'), 'utf8');
  new Function('module', 'exports', 'require', source)(module, module.exports, require);
  return module.exports;
};
const configTailwind = { ...chargerTailwind() };
configTailwind.content = [`${portailN}/**/*.{js,jsx}`];
configTailwind.plugins = [require('@tailwindcss/typography')];

export default defineConfig({
  plugins: [
    {
      name: 'portail-substitutions',
      enforce: 'pre',
      async resolveId(source, importer, options) {
        if (!importer || !norme(importer).startsWith(`${portailN}/`)) return null;
        if (source.startsWith('.')) {
          const cible = norme(path.resolve(path.dirname(importer), source)).replace(/\.(js|jsx)$/, '');
          if (!cible.startsWith(`${portailN}/`)) return null;
          const remplacant = SUBSTITUTIONS[cible.slice(portailN.length + 1)];
          return remplacant ? path.join(ici, remplacant) : null;
        }
        if (source.startsWith('/') || source.startsWith('\0') || source.startsWith('@portail') || source.startsWith('@aurore')) return null;
        // Paquets npm importés par le portail : ceux de CE projet (frontend/ n'a pas de node_modules ici).
        return this.resolve(source, path.join(ici, 'index.html'), { ...options, skipSelf: true });
      },
    },
    {
      // Le portail écrit son JSX dans des fichiers .js.
      name: 'portail-js-as-jsx',
      async transform(code, id) {
        if (!norme(id).startsWith(`${portailN}/`) || !id.endsWith('.js')) return null;
        return transformWithEsbuild(code, id, { loader: 'jsx', jsx: 'automatic' });
      },
    },
    react(),
  ],
  resolve: {
    alias: { '@aurore': aurore, '@portail': portail },
    dedupe: ['react', 'react-dom', '@remixicon/react', 'react-router-dom', 'axios'],
  },
  css: { postcss: { plugins: [tailwindcss(configTailwind), autoprefixer()] } },
  optimizeDeps: { esbuildOptions: { loader: { '.js': 'jsx' } } },
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
    'process.env.REACT_APP_API_URL': JSON.stringify(`${PORTAIL}/api`),
    __CES_PORTAIL__: JSON.stringify(PORTAIL),
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    fs: { allow: [ici, portail, path.resolve(ici, '../../brand')] },
  },
  build: {
    target: 'chrome120',
    outDir: 'dist',
    rollupOptions: { input: { main: path.join(ici, 'index.html'), site: path.join(ici, 'site.html'), annonce: path.join(ici, 'annonce.html') } },
  },
});
