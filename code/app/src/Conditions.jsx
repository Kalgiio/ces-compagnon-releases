// Conditions d'utilisation et données personnelles (demande du CES, 25/11). Même texte que la page « Conditions »
// de l'installeur (src-tauri/installer/conditions.txt) ; montré au lancement tant que la version en vigueur n'est pas
// acceptée sur ce PC, y compris pour les installations antérieures (1.0.0).
import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { exit } from '@tauri-apps/plugin-process';
import { Button, Message } from '@aurore/components';
import TEXTE from '../src-tauri/installer/conditions.txt?raw';

const LIEN = /(https?:\/\/[^\s)]+)/g;
function Lignes({ texte }) {
  return texte.split(LIEN).map((p, i) => (/^https?:\/\//.test(p)
    ? <a key={i} href={p} onClick={(e) => { e.preventDefault(); invoke('conditions_ouvrir_lien', { url: p }).catch(() => {}); }}>{p.replace(/^https:\/\//, '')}</a>
    : p));
}

/** Texte des conditions, mis en forme (titres « ## », listes « - »). */
export function TexteConditions() {
  const lignes = TEXTE.replace(/^﻿/, '').split(/\r?\n/);
  const blocs = [];
  let liste = null;
  lignes.forEach((l, i) => {
    if (l.startsWith('- ')) { (liste ??= []).push(<li key={i}><Lignes texte={l.slice(2)} /></li>); return; }
    if (liste) { blocs.push(<ul key={`u${i}`}>{liste}</ul>); liste = null; }
    if (i === 0) blocs.push(<h2 key={i}>{l}</h2>);
    else if (l.startsWith('## ')) blocs.push(<h3 key={i}>{l.slice(3)}</h3>);
    else if (l.trim()) blocs.push(<p key={i}><Lignes texte={l} /></p>);
  });
  if (liste) blocs.push(<ul key="fin">{liste}</ul>);
  return <div className="cgu-texte">{blocs}</div>;
}

/** Écran bloquant : accepter pour continuer, ou refuser et quitter. */
export function EcranConditions({ onAccepte }) {
  const [erreur, setErreur] = useState(null);
  const [fin, setFin] = useState(false);
  const accepter = () => invoke('conditions_accepter').then(onAccepte).catch((e) => setErreur(String(e)));
  return (
    <div className="cgu">
      <div className="cgu__carte" role="dialog" aria-modal="true" aria-label="Conditions d’utilisation">
        <p className="cgu__intro">Avant de continuer, lis les conditions d’utilisation de CES Compagnon : elles disent quelles données personnelles partent vers le portail du CES (dont ton adresse IP et l’heure de chaque échange), pourquoi, combien de temps, et tes droits.</p>
        <div className="cgu__defil" onScroll={(e) => { const t = e.currentTarget; if (t.scrollTop + t.clientHeight >= t.scrollHeight - 24) setFin(true); }}>
          <TexteConditions />
        </div>
        {erreur && <Message severity="danger">{erreur}</Message>}
        <div className="cgu__actions">
          <Button variant="text" label="Refuser et quitter" onClick={() => exit(0)} />
          <Button icon="check" label={fin ? 'J’accepte' : 'Lis jusqu’en bas pour accepter'} disabled={!fin} onClick={accepter} />
        </div>
      </div>
    </div>
  );
}
