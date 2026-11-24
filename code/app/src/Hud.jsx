// Habillage « HUD » de CES Compagnon (demande du CES, 21/11 : mélange des pistes « Cockpit » et « Tactique ») :
// barre visière, rail de navigation, fond choisi parmi ses captures, tableau de bord en tuiles déplaçables et
// fenêtres flottantes. Plus la fenêtre SOS et le bouton RSI Launcher.
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Icon, Button, Message } from '@aurore/components';
import rsiSvg from './assets/rsi.svg?raw';
import fondPolaris from './assets/fond-polaris.webp';
import fondMercury from './assets/fond-mercury.webp';
import fondNuit from './assets/fond-nuit.webp';

// --- Fonds ------------------------------------------------------------------------------------------------------
export const FONDS = [
  { id: 'polaris', label: 'Au-dessus des nuages', src: fondPolaris },
  { id: 'mercury', label: 'Planète enneigée', src: fondMercury },
  { id: 'nuit', label: 'Nuit en orbite', src: fondNuit },
  { id: 'aucun', label: 'Sans image', src: null },
];
const lire = (cle, defaut) => { try { return localStorage.getItem(cle) ?? defaut; } catch { return defaut; } };
const ecrire = (cle, v) => { try { localStorage.setItem(cle, v); } catch { /* navigateur intégré sans stockage */ } };

/** Fond choisi : une des captures, « sans image », ou l'image personnelle du membre (gardée sur ce PC, cœur Rust). */
export function useFond() {
  const [id, setId] = useState(() => lire('cmp-fond', 'polaris'));
  const [perso, setPerso] = useState(null);
  useEffect(() => { invoke('fond_perso_lire').then(setPerso).catch(() => {}); }, []);
  useEffect(() => { ecrire('cmp-fond', id); }, [id]);
  const choix = id === 'perso' ? (perso ? { id: 'perso', label: 'Mon image', src: perso } : FONDS[0]) : (FONDS.find((f) => f.id === id) || FONDS[0]);
  return [choix, setId, { perso, setPerso }];
}

/** Choix du fond (dans Outils) : captures, sans image, ou sa propre image. */
export function ChoixFond({ fond, setFond, perso, setPerso }) {
  const [erreur, setErreur] = useState(null);
  const choisirImage = async () => {
    setErreur(null);
    try {
      const url = await invoke('fond_perso_choisir');
      if (url) { setPerso(url); setFond('perso'); }
    } catch (e) { setErreur(String(e)); }
  };
  const retirer = () => { invoke('fond_perso_effacer').catch(() => {}); setPerso(null); if (fond.id === 'perso') setFond('polaris'); };
  return (
    <>
      <div className="hud-fonds" role="radiogroup" aria-label="Fond de l'application">
        {FONDS.map((f) => (
          <button key={f.id} type="button" role="radio" aria-checked={fond.id === f.id} className={fond.id === f.id ? 'hud-fond is-actif' : 'hud-fond'}
            onClick={() => setFond(f.id)}>
            <span className="hud-fond__img" style={f.src ? { backgroundImage: `url(${f.src})` } : undefined} />
            <span className="hud-fond__nom">{f.label}</span>
          </button>
        ))}
        <button type="button" role="radio" aria-checked={fond.id === 'perso'} className={fond.id === 'perso' ? 'hud-fond is-actif' : 'hud-fond'}
          onClick={() => (perso ? setFond('perso') : choisirImage())}>
          <span className="hud-fond__img" style={perso ? { backgroundImage: `url(${perso})` } : undefined}>{!perso && <Icon name="upload" size={22} />}</span>
          <span className="hud-fond__nom">{perso ? 'Mon image' : 'Ma propre image…'}</span>
        </button>
      </div>
      <div className="cmp-actions">
        <Button size="sm" variant="outlined" icon="upload" label={perso ? 'Changer mon image…' : 'Choisir une image…'} onClick={choisirImage} />
        {perso && <Button size="sm" variant="text" icon="trash" label="Retirer mon image" onClick={retirer} />}
      </div>
      <p className="cmp-muted cmp-small cmp-gap">Ton image reste sur ce PC (réduite à 2560 px de large). JPEG, PNG ou WebP.</p>
      {erreur && <div className="cmp-gap"><Message severity="danger" onClose={() => setErreur(null)}>{erreur}</Message></div>}
    </>
  );
}

// --- Petits éléments de la visière -----------------------------------------------------------------------------
export function RsiLogo({ className }) {
  return <span className={className} aria-hidden="true" dangerouslySetInnerHTML={{ __html: rsiSvg }} />;
}

export function Horloge() {
  const [maintenant, setMaintenant] = useState(() => new Date());
  useEffect(() => { const t = setInterval(() => setMaintenant(new Date()), 15000); return () => clearInterval(t); }, []);
  const hm = (d, utc) => d.toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit', timeZone: utc ? 'UTC' : undefined });
  return (
    <span className="hud-horloge" title="Heure locale · heure UTC (heure des serveurs)">
      <strong>{hm(maintenant)}</strong><small>UTC {hm(maintenant, true)}</small>
    </span>
  );
}

/** Bouton RSI Launcher : ouvre le lanceur officiel (l'application ne touche pas au jeu). */
export function BoutonRsi() {
  const [chemin, setChemin] = useState(undefined);
  const [bulle, setBulle] = useState(null);
  useEffect(() => { invoke('rsi_lanceur_etat').then(setChemin).catch(() => setChemin(null)); }, []);
  useEffect(() => { if (!bulle) return undefined; const t = setTimeout(() => setBulle(null), 5000); return () => clearTimeout(t); }, [bulle]);
  const ouvrir = async () => {
    try {
      if (!chemin) {
        const c = await invoke('rsi_lanceur_choisir');
        if (!c) return;
        setChemin(c);
      }
      await invoke('rsi_lanceur_ouvrir');
      setBulle({ ok: true, texte: 'RSI Launcher ouvert.' });
    } catch (e) { setBulle({ ok: false, texte: String(e) }); }
  };
  return (
    <span className="hud-rsi-zone">
      <button type="button" className="hud-rsi" onClick={ouvrir}
        title={chemin ? `Ouvrir le RSI Launcher (${chemin})` : 'RSI Launcher introuvable : choisir son emplacement'}
        aria-label="Ouvrir le RSI Launcher">
        <RsiLogo className="hud-rsi__logo" />
        <span>Launcher</span>
      </button>
      {bulle && <span className={bulle.ok ? 'hud-bulle' : 'hud-bulle is-erreur'} role="status">{bulle.texte}</span>}
    </span>
  );
}

// --- SOS ---------------------------------------------------------------------------------------------------------
const TYPES_SOS = [
  { id: 'combat', label: 'Combat', icon: 'sword' },
  { id: 'panne', label: 'Panne', icon: 'settings' },
  { id: 'carburant', label: 'Carburant', icon: 'alert' },
  { id: 'medical', label: 'Médical', icon: 'medic' },
  { id: 'transport', label: 'Transport', icon: 'truck' },
  { id: 'autre', label: 'Autre', icon: 'flag' },
];

/** État des SOS (portail) : relu à chaque `sos:update` et toutes les 60 s. */
export function useSos(relie, moi) {
  const [etat, setEtat] = useState(null);
  const [erreur, setErreur] = useState(null);
  const relire = useCallback(() => {
    if (!relie) return Promise.resolve();
    return invoke('sos_action', { action: 'etat' }).then((v) => { setEtat(v); setErreur(null); }).catch((e) => setErreur(String(e)));
  }, [relie]);
  useEffect(() => {
    if (!relie) return undefined;
    relire();
    const t = setInterval(relire, 60000);
    const fin = listen('sos-evenement', () => relire());
    return () => { clearInterval(t); fin.then((f) => f()); };
  }, [relie, relire]);
  const actives = etat?.active ?? [];
  const mien = actives.find((a) => moi && a.name === moi);
  return { etat, erreur, relire, actives, mien };
}

/** Gros bouton SOS : il faut le MAINTENIR 1 s (pas de déclenchement par erreur), puis la fenêtre SOS s'ouvre. */
export function BoutonSos({ onOuvrir, nbActifs, mien }) {
  const [charge, setCharge] = useState(false);
  const minuteur = useRef(null);
  const [aide, setAide] = useState(false);
  const debut = (e) => {
    if (e.button !== 0) return;
    setCharge(true);
    minuteur.current = setTimeout(() => { setCharge(false); minuteur.current = null; onOuvrir(); }, 1000);
  };
  const fin = () => {
    if (minuteur.current) { clearTimeout(minuteur.current); minuteur.current = null; setAide(true); setTimeout(() => setAide(false), 2200); }
    setCharge(false);
  };
  return (
    <span className="hud-sos-zone">
      <button type="button" className={`hud-sos${charge ? ' is-charge' : ''}${mien ? ' is-mien' : ''}`}
        onPointerDown={debut} onPointerUp={fin} onPointerLeave={fin}
        onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onOuvrir(); } }}
        title="SOS : maintenir 1 seconde pour appeler le CES" aria-label="SOS : maintenir une seconde pour appeler le CES">
        <span className="hud-sos__jauge" aria-hidden="true" />
        <Icon name="alert" size={15} />
        <span>SOS</span>
        {nbActifs > 0 && <i className="hud-sos__nb" aria-label={`${nbActifs} SOS en cours`}>{nbActifs}</i>}
      </button>
      {aide && <span className="hud-bulle" role="status">Maintiens le bouton 1 seconde.</span>}
    </span>
  );
}

const heure = (iso) => (iso ? new Date(iso).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' }) : '');

function CarteSos({ a, moi, onAction, occupe }) {
  const mienne = moi && a.name === moi;
  const jyVais = (a.responders || []).some((r) => r.name === moi);
  return (
    <li className={mienne ? 'hud-alerte is-mienne' : 'hud-alerte'}>
      <div className="hud-alerte__tete">
        <span className="hud-alerte__type">{a.kindEmoji} {a.kindLabel || a.kind}</span>
        <strong>{mienne ? 'Ton SOS' : a.name}</strong>
        <time>{heure(a.createdAt)}</time>
      </div>
      {(a.location || a.system) && <p className="hud-alerte__lieu"><Icon name="pin" size={13} /> {[a.location, a.system].filter(Boolean).join(' · ')}</p>}
      {a.message && <p className="hud-alerte__msg">{a.message}</p>}
      <p className="cmp-muted cmp-small">
        {(a.responders || []).length ? `En route : ${a.responders.map((r) => r.name).join(', ')}` : 'Personne en route pour l’instant.'}
      </p>
      <div className="cmp-actions">
        {mienne ? (<>
          <Button size="sm" label="Résolu" icon="check" disabled={occupe} onClick={() => onAction('clore', a.id)} />
          <Button size="sm" variant="outlined" label="Annuler mon SOS" disabled={occupe} onClick={() => onAction('annuler', a.id)} />
        </>) : (<>
          <Button size="sm" label={jyVais ? 'Je n’y vais plus' : 'J’y vais'} icon={jyVais ? 'undo' : 'rocket'} variant={jyVais ? 'outlined' : undefined} disabled={occupe} onClick={() => onAction('repondre', a.id)} />
          {jyVais && <Button size="sm" variant="text" label="Résolu" disabled={occupe} onClick={() => onAction('clore', a.id)} />}
        </>)}
      </div>
    </li>
  );
}

/** Contenu de la fenêtre SOS : alertes en cours + lancer un SOS (type, message, confirmation). */
export function PanneauSos({ sos, moi }) {
  const [type, setType] = useState(null);
  const [message, setMessage] = useState('');
  const [confirmer, setConfirmer] = useState(false);
  const [occupe, setOccupe] = useState(false);
  const [retour, setRetour] = useState(null);
  const action = async (quoi, id) => {
    setOccupe(true); setRetour(null);
    try {
      if (quoi === 'repondre') await invoke('sos_action', { action: 'repondre', id });
      else await invoke('sos_action', { action: 'clore', id, annuler: quoi === 'annuler' });
      await sos.relire();
    } catch (e) { setRetour({ severity: 'danger', texte: String(e) }); }
    finally { setOccupe(false); }
  };
  const envoyer = async () => {
    setOccupe(true); setRetour(null);
    try {
      await invoke('sos_action', { action: 'envoyer', kind: type, message });
      setType(null); setMessage(''); setConfirmer(false);
      setRetour({ severity: 'success', texte: 'SOS envoyé au CES : les membres sont prévenus (portail, Discord, application).' });
      await sos.relire();
    } catch (e) { setRetour({ severity: 'danger', texte: String(e) }); setConfirmer(false); }
    finally { setOccupe(false); }
  };
  return (
    <div className="hud-sos-panneau">
      {sos.erreur && <Message severity="warn">{sos.erreur}</Message>}
      {retour && <Message severity={retour.severity} onClose={() => setRetour(null)}>{retour.texte}</Message>}
      {sos.actives.length > 0 && (
        <ul className="hud-alertes">
          {sos.actives.map((a) => <CarteSos key={a.id} a={a} moi={moi} onAction={action} occupe={occupe} />)}
        </ul>
      )}
      {sos.etat?.myBlock && (
        <Message severity="warn" title="SOS bloqués par la modération">
          {sos.etat.myBlock.until && new Date(sos.etat.myBlock.until).getFullYear() < 9000 ? `Jusqu’au ${new Date(sos.etat.myBlock.until).toLocaleString('fr-FR')}.` : 'Sans date de fin.'}
          {sos.etat.myBlock.reason ? ` Motif : ${sos.etat.myBlock.reason}` : ''}
        </Message>
      )}
      {!sos.mien && !sos.etat?.myBlock && (
        <section className="hud-sos-nouveau">
          <h4 className="hud-titre">Appeler le CES</h4>
          <div className="hud-sos-types" role="radiogroup" aria-label="Type de SOS">
            {TYPES_SOS.map((t) => (
              <button key={t.id} type="button" role="radio" aria-checked={type === t.id} className={type === t.id ? 'hud-sos-type is-actif' : 'hud-sos-type'}
                onClick={() => { setType(t.id); setConfirmer(false); }}>
                <Icon name={t.icon} size={18} /><span>{t.label}</span>
              </button>
            ))}
          </div>
          <textarea className="hud-saisie" rows={2} maxLength={500} placeholder="Message (facultatif) : ce qui se passe, ce dont tu as besoin…"
            value={message} onChange={(e) => setMessage(e.target.value)} />
          <p className="cmp-muted cmp-small">Ton lieu est ajouté par le portail d’après ta dernière position connue (moins de 2 h).</p>
          {!confirmer
            ? <div className="cmp-actions"><Button label="Envoyer le SOS…" icon="alert" disabled={!type || occupe} onClick={() => setConfirmer(true)} /></div>
            : (
              <div className="hud-confirmer">
                <p><strong>Envoyer un SOS « {TYPES_SOS.find((t) => t.id === type)?.label} » à tout le CES ?</strong></p>
                <div className="cmp-actions">
                  <Button variant="outlined" label="Annuler" disabled={occupe} onClick={() => setConfirmer(false)} />
                  <Button label="Oui, envoyer" icon="send" loading={occupe} onClick={envoyer} />
                </div>
              </div>
            )}
        </section>
      )}
      {sos.etat?.recent?.length > 0 && (
        <details className="hud-sos-recents">
          <summary className="cmp-small">SOS récents ({sos.etat.recent.length})</summary>
          <ul>{sos.etat.recent.slice(0, 8).map((a) => (
            <li key={a.id} className="cmp-small"><span>{a.kindEmoji} {a.name}</span><span className="cmp-muted">{a.status === 'resolved' ? 'résolu' : a.status === 'false' ? 'faux SOS' : 'annulé'} · {heure(a.closedAt || a.createdAt)}</span></li>
          ))}</ul>
        </details>
      )}
    </div>
  );
}

// --- Fenêtre flottante -------------------------------------------------------------------------------------------
/** Petite fenêtre déplaçable par sa barre de titre (position mémorisée, toujours dans l'écran). */
export function FenetreFlottante({ cle, titre, icone, ton, onFermer, children, largeur = 420 }) {
  const [pos, setPos] = useState(() => {
    try { const p = JSON.parse(lire(`cmp-fen-${cle}`, 'null')); if (p) return p; } catch { /* ignoré */ }
    return { x: Math.max(16, window.innerWidth - largeur - 28), y: 96 };
  });
  const ref = useRef(null);
  const borner = useCallback((p) => {
    const l = ref.current?.offsetWidth || largeur;
    return { x: Math.min(Math.max(4, p.x), window.innerWidth - l - 4), y: Math.min(Math.max(50, p.y), window.innerHeight - 60) };
  }, [largeur]);
  useEffect(() => {
    const auRedim = () => setPos((p) => borner(p));
    window.addEventListener('resize', auRedim);
    return () => window.removeEventListener('resize', auRedim);
  }, [borner]);
  useEffect(() => { ecrire(`cmp-fen-${cle}`, JSON.stringify(pos)); }, [cle, pos]);
  useEffect(() => {
    const echap = (e) => { if (e.key === 'Escape') onFermer(); };
    window.addEventListener('keydown', echap);
    return () => window.removeEventListener('keydown', echap);
  }, [onFermer]);
  const saisir = (e) => {
    if (e.button !== 0 || e.target.closest('button')) return;
    e.preventDefault();
    const depart = { x: e.clientX - pos.x, y: e.clientY - pos.y };
    const bouger = (ev) => setPos(borner({ x: ev.clientX - depart.x, y: ev.clientY - depart.y }));
    const lacher = () => { window.removeEventListener('pointermove', bouger); window.removeEventListener('pointerup', lacher); };
    window.addEventListener('pointermove', bouger);
    window.addEventListener('pointerup', lacher);
  };
  return (
    <div ref={ref} className={`hud-fenetre${ton ? ` hud-fenetre--${ton}` : ''}`} style={{ left: pos.x, top: pos.y, width: largeur }} role="dialog" aria-label={titre}>
      <div className="hud-fenetre__barre" onPointerDown={saisir}>
        <Icon name="drag" size={14} />
        {icone && (typeof icone === 'string' ? <Icon name={icone} size={15} /> : icone)}
        <strong>{titre}</strong>
        <button type="button" className="hud-fenetre__fermer" aria-label="Fermer" title="Fermer (Échap)" onClick={onFermer}><Icon name="close" size={14} /></button>
      </div>
      <div className="hud-fenetre__corps">{children}</div>
    </div>
  );
}

// --- Tableau de bord en tuiles (grille 12 colonnes) --------------------------------------------------------------
const COLONNES = 12;
const RANG = 34; // hauteur d'un rang (px)
const ECART = 12;

const chevauche = (a, b) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

/** Place `fixe`, puis descend les tuiles qui le chevauchent (et ainsi de suite), puis remonte tout au plus haut. */
function ranger(dispo, fixeId) {
  const liste = dispo.map((t) => ({ ...t }));
  const fixe = liste.find((t) => t.id === fixeId);
  const ordre = [fixe, ...liste.filter((t) => t !== fixe).sort((a, b) => a.y - b.y || a.x - b.x)].filter(Boolean);
  const places = [];
  for (const t of ordre) {
    if (t !== fixe) {
      // remonte au plus haut sans chevaucher ce qui est déjà placé
      let y = 0;
      while (places.some((p) => chevauche({ ...t, y }, p))) y += 1;
      t.y = y;
    }
    places.push(t);
  }
  return liste;
}

export function TableauDeBord({ cle, tuiles }) {
  const defaut = useCallback(() => tuiles.map((t) => ({ id: t.id, ...t.defaut })), [tuiles]);
  const cacheesDefaut = tuiles.filter((t) => t.cachee).map((t) => t.id);
  const [dispo, setDispo] = useState(() => {
    try {
      const d = JSON.parse(lire(cle, 'null'));
      if (Array.isArray(d) && d.every((x) => tuiles.some((t) => t.id === x.id))) {
        const manquantes = tuiles.filter((t) => !d.some((x) => x.id === t.id) && !t.cachee).map((t) => ({ id: t.id, ...t.defaut }));
        return [...d, ...manquantes];
      }
    } catch { /* ignoré */ }
    return defaut();
  });
  const [cachees, setCachees] = useState(() => { try { return JSON.parse(lire(`${cle}-cachees`, 'null')) ?? cacheesDefaut; } catch { return cacheesDefaut; } });
  useEffect(() => { ecrire(cle, JSON.stringify(dispo)); }, [cle, dispo]);
  useEffect(() => { ecrire(`${cle}-cachees`, JSON.stringify(cachees)); }, [cle, cachees]);
  const grille = useRef(null);
  const [geste, setGeste] = useState(null); // { id, mode: 'deplacer'|'taille', apercu: {x,y,w,h} }
  const [etroit, setEtroit] = useState(false);
  useEffect(() => {
    if (!grille.current) return undefined;
    const obs = new ResizeObserver(([e]) => setEtroit(e.contentRect.width < 760));
    obs.observe(grille.current);
    return () => obs.disconnect();
  }, []);

  const cellule = (clientX, clientY) => {
    const r = grille.current.getBoundingClientRect();
    const col = (r.width + ECART) / COLONNES;
    return { cx: (clientX - r.left) / col, cy: (clientY - r.top + grille.current.scrollTop) / (RANG + ECART) };
  };
  const commencer = (e, id, mode) => {
    if (e.button !== 0 || etroit) return;
    if (mode === 'deplacer' && (!e.target.closest('.ds-panel__head, .psl-bloc__tete') || e.target.closest('button, a, input, select, textarea'))) return;
    e.preventDefault();
    const t = dispo.find((x) => x.id === id);
    const depart = cellule(e.clientX, e.clientY);
    let apercu = { ...t };
    setGeste({ id, mode, apercu });
    const bouger = (ev) => {
      const c = cellule(ev.clientX, ev.clientY);
      const dx = Math.round(c.cx - depart.cx);
      const dy = Math.round(c.cy - depart.cy);
      apercu = mode === 'deplacer'
        ? { ...t, x: Math.min(Math.max(0, t.x + dx), COLONNES - t.w), y: Math.max(0, t.y + dy) }
        : { ...t, w: Math.min(Math.max(3, t.w + dx), COLONNES - t.x), h: Math.max(4, t.h + dy) };
      setGeste({ id, mode, apercu });
    };
    const lacher = () => {
      window.removeEventListener('pointermove', bouger);
      window.removeEventListener('pointerup', lacher);
      setDispo((d) => ranger(d.map((x) => (x.id === id ? apercu : x)), id));
      setGeste(null);
    };
    window.addEventListener('pointermove', bouger);
    window.addEventListener('pointerup', lacher);
  };

  const visibles = dispo.filter((d) => !cachees.includes(d.id));
  const cacher = (id) => setCachees((c) => [...new Set([...c, id])]);
  const montrer = (id) => {
    setCachees((c) => c.filter((x) => x !== id));
    setDispo((d) => (d.some((x) => x.id === id) ? d : [...d, { id, ...tuiles.find((t) => t.id === id).defaut }]));
  };
  const style = (p) => (etroit ? undefined : { gridColumn: `${p.x + 1} / span ${p.w}`, gridRow: `${p.y + 1} / span ${p.h}` });

  return (
    <div className="hud-tdb">
      <div className="hud-tdb__outils">
        <span className="hud-titre"><Icon name="drag" size={13} /> Glisse une fenêtre par son titre, agrandis-la par son coin.</span>
        <span className="hud-tdb__ajouts">
          {cachees.map((id) => {
            const t = tuiles.find((x) => x.id === id);
            return t ? <Button key={id} size="sm" variant="text" icon="plus" label={t.titre} onClick={() => montrer(id)} /> : null;
          })}
          <Button size="sm" variant="text" icon="undo" label="Disposition d’origine" onClick={() => { setDispo(defaut()); setCachees(cacheesDefaut); }} />
        </span>
      </div>
      <div ref={grille} className={etroit ? 'hud-grille is-etroite' : 'hud-grille'} style={{ '--rang': `${RANG}px`, '--ecart': `${ECART}px` }}>
        {visibles.map((p) => {
          const t = tuiles.find((x) => x.id === p.id);
          if (!t) return null;
          const enGeste = geste?.id === p.id;
          return (
            <div key={p.id} className={enGeste ? 'hud-tuile is-geste' : 'hud-tuile'} style={style(p)} onPointerDown={(e) => commencer(e, p.id, 'deplacer')}>
              {t.rendu()}
              <button type="button" className="hud-tuile__cacher" aria-label={`Masquer ${t.titre}`} title="Masquer" onClick={() => cacher(p.id)}><Icon name="close" size={12} /></button>
              {!etroit && <span className="hud-tuile__taille" aria-hidden="true" onPointerDown={(e) => { e.stopPropagation(); commencer(e, p.id, 'taille'); }} />}
            </div>
          );
        })}
        {geste && !etroit && <div className="hud-tuile-apercu" style={style(geste.apercu)} aria-hidden="true" />}
      </div>
    </div>
  );
}



/** Image en grand (clic sur une image d'objet) : fond assombri, Échap ou clic pour fermer. */
export function Visionneuse({ image, onFermer }) {
  useEffect(() => {
    if (!image) return undefined;
    const echap = (e) => { if (e.key === 'Escape') onFermer(); };
    window.addEventListener('keydown', echap);
    return () => window.removeEventListener('keydown', echap);
  }, [image, onFermer]);
  if (!image) return null;
  return (
    <div className="hud-visio" role="dialog" aria-label={image.titre || 'Image'} onClick={onFermer}>
      <figure onClick={(e) => e.stopPropagation()}>
        <img src={image.src} alt={image.titre || ''} />
        {image.titre && <figcaption>{image.titre}{image.sous ? <small>{image.sous}</small> : null}</figcaption>}
      </figure>
      <button type="button" className="hud-visio__fermer" aria-label="Fermer" onClick={onFermer}><Icon name="close" size={18} /></button>
    </div>
  );
}
