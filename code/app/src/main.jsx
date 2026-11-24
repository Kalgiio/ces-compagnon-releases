import React from 'react';
import { createRoot } from 'react-dom/client';
import '@fontsource-variable/dm-sans';
import '@fontsource/ubuntu-mono/latin-400.css';
import '@fontsource/orbitron/latin-700.css';
import App from './App.jsx';
import './app.css';

// Thème posé AVANT le premier rendu (pas de flash) : choix mémorisé (Système / Clair / Sombre), sinon Windows.
// Aurore Design lit l'attribut `data-mui-color-scheme` (nom hérité du portail). La suite est gérée par useTheme().
{
  let choix = 'systeme';
  try { choix = localStorage.getItem('cmp-theme') || 'systeme'; } catch { /* ignoré */ }
  const sombre = choix === 'sombre' || (choix === 'systeme' && window.matchMedia('(prefers-color-scheme: dark)').matches);
  document.documentElement.setAttribute('data-mui-color-scheme', sombre ? 'dark' : 'light');
}

createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
