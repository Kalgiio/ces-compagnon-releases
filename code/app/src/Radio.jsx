// Radio du CES dans l'application (demande du CES, 23/11) : même antenne que le portail (/api/radio), EN PAUSE au
// démarrage — rien ne joue sans clic. Lecteur vinyle dans la navigation + fenêtre flottante (choix du CES : les deux).
//
// Direct : tout le monde entend le même morceau à la même seconde. Position = (heure serveur corrigée − startedAt) ;
// l'écart d'horloge est mesuré à chaque appel (serverNow − milieu de l'aller-retour), jamais l'horloge du PC seule.
// Recalage toutes les 20 s (?listening=1 compte l'auditeur) : écart > 3 s → on se recale ; autre morceau → on change.
// Le son passe par le relais `cesmedia` du cœur (jeton, plages HTTP de 2 Mo).
import React, { useEffect, useSyncExternalStore } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Button } from '@aurore/components';
import { RiPlayFill, RiPauseFill, RiVolumeUpLine, RiVolumeMuteLine } from '@remixicon/react';

const MEDIA_API = 'http://cesmedia.localhost/__api';
const src = (chemin) => (chemin ? `${MEDIA_API}${chemin.startsWith('/') ? '' : '/'}${chemin}` : null);
const lireLS = (k, d) => { try { const v = localStorage.getItem(k); return v === null ? d : JSON.parse(v); } catch { return d; } };
const ecrireLS = (k, v) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch { /* ignoré */ } };

// --- Moteur (hors React : la musique continue d'un écran à l'autre) ----------------------------------------------
const audio = typeof Audio !== 'undefined' ? new Audio() : null;
const abonnes = new Set();
let etat = { live: null, morceau: null, enLecture: false, charge: false, erreur: null, volume: lireLS('cmp-radio-volume', 0.6), muet: false, auditeurs: 0, position: 0, duree: 0 };
let ecart = 0; // heure serveur − heure du PC (ms)
let minuteur = null;

const emettre = (p) => {
  const avant = etat;
  etat = { ...etat, ...p };
  // Bouton de l'aperçu de la barre des tâches et commandes multimédia de Windows (24/11).
  if (avant.enLecture !== etat.enLecture) {
    invoke('vignette_radio', { enLecture: etat.enLecture }).catch(() => {});
    if (navigator.mediaSession) navigator.mediaSession.playbackState = etat.enLecture ? 'playing' : 'paused';
  }
  if (avant.morceau?.id !== etat.morceau?.id && etat.morceau && navigator.mediaSession && typeof MediaMetadata !== 'undefined') {
    navigator.mediaSession.metadata = new MediaMetadata({ title: etat.morceau.title, artist: etat.morceau.artist || 'Radio du CES', album: etat.morceau.album || 'CES Compagnon' });
  }
  abonnes.forEach((f) => f());
};
if (audio) {
  audio.preload = 'none';
  audio.volume = etat.volume;
  audio.addEventListener('timeupdate', () => emettre({ position: audio.currentTime, duree: audio.duration || etat.morceau?.durationSec || 0 }));
  audio.addEventListener('waiting', () => emettre({ charge: true }));
  audio.addEventListener('playing', () => emettre({ charge: false, enLecture: true, erreur: null }));
  audio.addEventListener('pause', () => emettre({ enLecture: false }));
  audio.addEventListener('error', () => emettre({ charge: false, enLecture: false, erreur: 'Lecture impossible (connexion au portail ?).' }));
  // Morceau fini avant le serveur (durées arrondies) : enchaîner sur le suivant, puis recaler.
  audio.addEventListener('ended', () => {
    const suivant = etat.live?.next;
    if (suivant) { charger(suivant, 0); audio.play().catch(() => {}); }
    setTimeout(() => synchro(true), 1500);
  });
}

async function lireDirect(ecoute) {
  const t0 = Date.now();
  const v = await invoke('portail_api', { methode: 'GET', chemin: ecoute ? 'radio/live?listening=1' : 'radio/live', corps: null });
  const t1 = Date.now();
  if (v?.serverNow) ecart = v.serverNow - (t0 + t1) / 2;
  emettre({ live: v?.live || null, auditeurs: v?.listeners || 0 });
  return v?.live || null;
}
const positionDirect = (live) => Math.max(0, (Date.now() + ecart - live.startedAt) / 1000);

function charger(morceau, position) {
  if (!audio || !morceau) return;
  const url = src(morceau.audio);
  if (audio.dataset.id !== morceau.id) {
    audio.src = url;
    audio.dataset.id = morceau.id;
    emettre({ morceau, position: 0, duree: morceau.durationSec || 0 });
  }
  const caler = () => { try { audio.currentTime = position; } catch { /* pas encore prêt */ } };
  if (audio.readyState >= 1) caler(); else audio.addEventListener('loadedmetadata', caler, { once: true });
}

async function synchro(force) {
  if (!audio || audio.paused) return;
  try {
    const live = await lireDirect(true);
    if (!live?.track) return;
    const pos = positionDirect(live);
    if (audio.dataset.id !== live.track.id) { charger(live.track, pos); audio.play().catch(() => {}); return; }
    if (force || Math.abs(audio.currentTime - pos) > 3) audio.currentTime = pos;
  } catch { /* hors ligne : on garde ce qui joue */ }
}

export const radio = {
  async jouer() {
    if (!audio) return;
    emettre({ charge: true, erreur: null });
    try {
      const live = await lireDirect(true);
      if (!live?.track) { emettre({ charge: false, erreur: 'Rien à l’antenne pour le moment.' }); return; }
      charger(live.track, positionDirect(live));
      await audio.play();
      clearInterval(minuteur);
      minuteur = setInterval(() => synchro(false), 20000);
    } catch (e) { emettre({ charge: false, enLecture: false, erreur: String(e?.message || e) }); }
  },
  pause() { if (audio) audio.pause(); clearInterval(minuteur); },
  basculer() { return etat.enLecture ? radio.pause() : radio.jouer(); },
  volume(v) { const x = Math.min(1, Math.max(0, v)); if (audio) { audio.volume = x; audio.muted = false; } ecrireLS('cmp-radio-volume', x); emettre({ volume: x, muet: false }); },
  muet() { if (!audio) return; audio.muted = !audio.muted; emettre({ muet: audio.muted }); },
  rafraichir() { return lireDirect(false).catch(() => null); },
};

// Clic sur le bouton de l'aperçu de la barre des tâches (vignette.rs) et touches multimédia du clavier.
listen('radio-basculer', () => radio.basculer()).catch(() => {});
if (navigator.mediaSession) {
  try {
    navigator.mediaSession.setActionHandler('play', () => radio.jouer());
    navigator.mediaSession.setActionHandler('pause', () => radio.pause());
    navigator.mediaSession.setActionHandler('stop', () => radio.pause());
  } catch { /* commandes multimédia indisponibles */ }
}

export function useRadio() {
  return useSyncExternalStore((f) => { abonnes.add(f); return () => abonnes.delete(f); }, () => etat);
}

// --- Interface --------------------------------------------------------------------------------------------------
const mmss = (s) => { const t = Math.max(0, Math.floor(s || 0)); return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`; };

/** Étiquette du disque : pochette en image de fond, centrée, sur une étiquette « CES » (visible si pas de pochette). */
function Pochette({ morceau, taille }) {
  return (
    <span className="rad-pochette" style={{ width: taille, height: taille }}>
      <span className="rad-pochette__ces">CES</span>
      {morceau?.cover && <span className="rad-pochette__img" style={{ backgroundImage: `url("${src(morceau.cover)}")` }} />}
    </span>
  );
}

/** Disque de la navigation : clic = fenêtre de la radio ; bouton = lecture / pause. */
export function VinyleNav({ onOuvrir, compact }) {
  const r = useRadio();
  useEffect(() => { radio.rafraichir(); }, []);
  const m = r.morceau || r.live?.track;
  return (
    <div className={`rad-nav${compact ? ' is-compact' : ''}${r.enLecture ? ' is-on' : ''}`}>
      <button type="button" className="rad-disque" onClick={onOuvrir} title={m ? `Radio du CES — ${m.title}` : 'Radio du CES'} aria-label="Ouvrir la radio du CES">
        <span className="rad-disque__vinyle"><Pochette morceau={m} taille={22} /></span>
      </button>
      {!compact && (
        <button type="button" className="rad-nav__lecture" onClick={radio.basculer} aria-label={r.enLecture ? 'Mettre la radio en pause' : 'Écouter la radio'} title={r.enLecture ? 'Pause' : 'Écouter'}>
          {r.enLecture ? <RiPauseFill size={14} /> : <RiPlayFill size={14} />}
        </button>
      )}
    </div>
  );
}

/** Contenu de la fenêtre flottante : en cours, commandes, sillon, volume, à suivre. */
export function PanneauRadio() {
  const r = useRadio();
  useEffect(() => { radio.rafraichir(); const t = setInterval(() => radio.rafraichir(), 30000); return () => clearInterval(t); }, []);
  const m = r.morceau || r.live?.track;
  const duree = r.duree || m?.durationSec || 0;
  const pos = r.enLecture ? r.position : (r.live ? positionDirect(r.live) : 0);
  return (
    <div className="rad-panneau">
      <div className="rad-tete">
        <span className={`rad-grand${r.enLecture ? ' is-on' : ''}`}><Pochette morceau={m} taille={74} /></span>
        <div className="rad-tete__txt">
          <span className="rad-direct"><i /> {r.live ? 'À L’ANTENNE' : 'ANTENNE COUPÉE'}{r.auditeurs ? ` · ${r.auditeurs} à l’écoute` : ''}</span>
          <strong>{m?.title || 'Radio du CES'}</strong>
          <small className="cmp-muted">{[m?.artist, m?.album].filter(Boolean).join(' · ') || 'Musique du Corps Expéditionnaire Spatial'}</small>
        </div>
      </div>
      <div className="rad-sillon" aria-hidden="true">
        {(m?.peaks || Array.from({ length: 48 }, () => 30)).filter((_, i, a) => a.length <= 48 || i % 2 === 0).map((p, i, a) => (
          <i key={i} style={{ height: `${Math.max(8, p)}%` }} className={duree && i / a.length < pos / duree ? 'is-passe' : undefined} />
        ))}
      </div>
      <div className="rad-temps cmp-small cmp-muted"><span>{mmss(pos)}</span><span>{mmss(duree)}</span></div>
      <div className="rad-commandes">
        <Button label={r.charge ? 'Connexion…' : r.enLecture ? 'Pause' : 'Écouter le direct'} loading={r.charge} onClick={radio.basculer} />
        <button type="button" className="rad-muet" onClick={radio.muet} aria-label={r.muet ? 'Remettre le son' : 'Couper le son'} title={r.muet ? 'Remettre le son' : 'Couper le son'}>
          {r.muet || r.volume === 0 ? <RiVolumeMuteLine size={16} /> : <RiVolumeUpLine size={16} />}
        </button>
        <input className="rad-volume" type="range" min="0" max="100" value={Math.round((r.muet ? 0 : r.volume) * 100)} aria-label="Volume"
          onChange={(e) => radio.volume(Number(e.target.value) / 100)} />
      </div>
      {r.erreur && <p className="cmp-small rad-erreur">{r.erreur}</p>}
      {r.live?.upcoming?.length > 0 && (
        <div>
          <h4 className="hud-titre">Ensuite</h4>
          <ol className="rad-suite">{r.live.upcoming.slice(0, 4).map((t) => <li key={t.id}><span>{t.title}</span><small className="cmp-muted">{t.artist || ''}</small></li>)}</ol>
        </div>
      )}
      <p className="cmp-muted cmp-small">En pause au démarrage. Le même morceau passe au même moment pour tout le CES (portail compris).</p>
    </div>
  );
}
