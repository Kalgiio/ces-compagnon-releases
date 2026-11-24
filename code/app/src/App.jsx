import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Channel, invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { DsRoot, Icon, Tag, Panel, Message, Status, EmptyState, Button, Field, InputText, SettingRow, ToggleSwitch, Dialog, SegmentedControl, Checkbox, Select, ProgressBar } from '@aurore/components';
import { DefconBar, Drawer, ModuleHero, Popover } from '@aurore/advanced';
import { relativeTime } from '@portail/utils/dateFormat';
import '@portail/components/NotificationBell.css';
import StarMap from '@portail/components/StarMap';
import { Missions, Forum, Annuaire, MesDonnees, Defcon, CeSoir, Contrats, Notifications, api as apiPortail, media as mediaPortail } from './Natif.jsx';
import { VinyleNav, PanneauRadio, radio } from './Radio.jsx';
import { BandeauPasserelle, BlocOperation, BlocAlerte, BlocEnJeu, BlocCeSoir, BlocRadio, BlocOutils } from './Passerelle.jsx';
import { MenuContextuel } from './Contexte.jsx';
import { RiMusic2Line } from '@remixicon/react';
import { Personnages } from './Personnages.jsx';
import { Visionneuse, useFond, ChoixFond, Horloge, BoutonRsi, BoutonSos, useSos, PanneauSos, FenetreFlottante, TableauDeBord } from './Hud.jsx';
import './hud.css';
import LICENCES from './licences.json';
import logoSombre from './assets/logo-compagnon-sombre.png';
import logoClair from './assets/logo-compagnon-clair.png';
import uexBadge from './assets/uex-powered.png';
import WIKELO_FR from './data/wikelo-fr.json';
// Logo du CES (organisation), v3 : marque du portail, distinct du logo de l'application (brand/, source unique).
import logoCes from '../../../brand/logo-ces-v3.svg';
import logoCesTexte from '../../../brand/logo-ces-v3-avec-texte.svg';

/** Logo « CES COMPAGNON » : texte clair en thème sombre, bleu en thème clair (le CSS affiche le bon). */
function LogoCompagnon({ className }) {
  return (
    <span className={`cmp-logo ${className || ''}`}>
      <img src={logoSombre} alt="CES Compagnon" className="cmp-logo__sombre" />
      <img src={logoClair} alt="" aria-hidden="true" className="cmp-logo__clair" />
    </span>
  );
}
import iconeCanary from '../icons-src/ces-compagnon-canary.svg';
import iconeStable from '../icons-src/ces-compagnon-stable.svg';

// Écrans de la v1 (ARCHITECTURE.md § 5). Seul « Accueil » est branché dans ce squelette.
// Entrées du portail intégré (sous un séparateur dans la barre latérale).
// Modules du portail gardés dans l'application, tous NATIFS (demande du CES, 21/11) : le hub, le planning, la gestion,
// le bot, l'académie et le garage ne sont plus dans l'application (ils restent sur le site). `visiteur` = ouvert au
// visiteur autorisé (join_missions) ; le serveur décide de toute façon.
const PORTAIL = [
  { id: 'missions', label: 'Missions', icon: 'rocket', visiteur: true, eyebrow: 'Opérations', titre: 'Missions', desc: 'Les opérations du CES : briefing, postes et inscription.' },
  { id: 'cesoir', label: 'Ce soir', icon: 'calendar', visiteur: true, eyebrow: 'Communauté', titre: 'Ce soir', desc: 'Les soirées proposées par les membres : rejoins-en une ou propose la tienne.' },
  { id: 'contrats', label: 'Contrats', icon: 'file', visiteur: true, eyebrow: 'Jeu', titre: 'Contrats du jeu', desc: 'Les contrats de Star Citizen en français : récompenses, conditions et objets.' },
  { id: 'forum', label: 'Forum', icon: 'forum', visiteur: true, eyebrow: 'Communauté', titre: 'Forum', desc: 'Les discussions du CES, catégorie par catégorie.' },
  { id: 'annuaire', label: 'Annuaire', icon: 'users', eyebrow: 'Communauté', titre: 'Annuaire', desc: 'Les membres du CES, leurs grades et leurs métiers.' },
  { id: 'defcon', label: 'DEFCON', icon: 'shield', visiteur: true, eyebrow: 'État d’alerte', titre: 'DEFCON', desc: 'Niveau d’alerte de l’escadre et bulletins.' },
  { id: 'mesdonnees', label: 'Mes données', icon: 'chart', visiteur: true, eyebrow: 'CES Compagnon', titre: 'Mes données', desc: 'Ton temps de jeu, ce que l’application a envoyé et qui est en jeu.' },
];

/** Lien du portail (notification…) → écran natif + élément ; `null` = page restée sur le site (ouverte dans le navigateur). */
function ecranDeLien(url) {
  const u = String(url || '');
  let m;
  if ((m = /^\/event\/([\w-]+)/.exec(u))) return ['missions', m[1]];
  if (/^\/(mission|create$|edit-mission)/.test(u)) return ['missions', null];
  if ((m = /^\/forum\/sujet\/([\w-]+)/.exec(u))) return ['forum', m[1]];
  if (/^\/forum/.test(u)) return ['forum', null];
  if ((m = /^\/profils\/(?:public\/)?member\/([\w-]+)/.exec(u))) return ['annuaire', m[1]];
  if (/^\/profils/.test(u)) return ['annuaire', null];
  if (/^\/compagnon/.test(u)) return ['mesdonnees', null];
  if (/^\/wikelo/.test(u)) return ['wikelo', null];
  if (/^\/ce-soir/.test(u)) return ['cesoir', null];
  if (/^\/(hub)?$/.test(u)) return ['accueil', null];
  return null;
}

/** Indicateurs de l'en-tête d'un écran local (DS-CES-01) ; rien quand la donnée manque. */
function statsEcran(id, etat, compte) {
  if (id === 'accueil' && etat) {
    return [
      { label: 'Star Citizen', value: etat.presence === 'offline' ? 'Hors ligne' : 'En jeu', icon: 'rocket' },
      etat.version && { label: 'Version du jeu', value: etat.version.split('.').slice(0, 3).join('.'), icon: 'info' },
      compte?.relie && { label: 'Portail', value: compte.partagePresence === false ? 'Suspendu' : 'Relié', icon: 'link' },
    ].filter(Boolean).map((x) => ({ ...x, value: String(x.value) }));
  }
  if (id === 'compte' && compte?.utilisateur) {
    return [{ label: 'Membre', value: compte.utilisateur.username, icon: 'user' }].concat(compte.utilisateur.matricule ? [{ label: 'Matricule', value: compte.utilisateur.matricule, icon: 'shield' }] : []);
  }
  return [];
}

/** Permissions effectives (union des rôles), comme frontend/src/utils/permissions.js. */
function permissions(u) {
  const roles = Array.isArray(u?.Roles) && u.Roles.length ? u.Roles : [u?.SiteRole].filter(Boolean);
  return roles.reduce((a, r) => ({ ...a, ...Object.fromEntries(Object.entries(r?.permissions || {}).filter(([, v]) => v === true)) }), {});
}

/** Modules du portail visibles pour ce compte (`compte.moi` = /api/me ; inconnu hors ligne → modules ouverts à tous). */
function modulesPortail(moi) {
  const membre = !moi || moi.role === 'admin' || moi.role === 'membre';
  return PORTAIL.filter((e) => membre || e.visiteur);
}
/** Gestion du DEFCON : admin ou permission manage_defcon (le portail revérifie). */
const peutDefcon = (moi) => moi?.role === 'admin' || permissions(moi).admin_access === true || permissions(moi).manage_defcon === true;

// Écrans locaux de l'application. `barre` : dans la navigation horizontale ; les autres sont dans le menu du compte.
// En-tête de chaque écran : DS-CES-01 (ModuleHero) d'Aurore Design, comme les modules du portail.
const ECRANS = [
  { id: 'accueil', label: 'Accueil', icon: 'home', barre: true, eyebrow: 'Poste de bord', titre: 'Accueil', desc: 'L’état du jeu sur ce PC et le lien avec le portail CES.' },
  { id: 'carnet', label: 'Carnet', icon: 'file', barre: true, eyebrow: 'Journal de bord', titre: 'Carnet de voyage', desc: 'Tes sessions de jeu et leurs étapes, gardées sur ce PC.' },
  { id: 'commerce', label: 'Commerce', icon: 'chart', barre: true, eyebrow: 'Activités', titre: 'Commerce', desc: 'Routes de commerce d’après les prix UEX, calculées sur ce PC.' },
  { id: 'minage', label: 'Minage', icon: 'ship', barre: true, eyebrow: 'Activités', titre: 'Minage', desc: 'Sessions de minage, raffinerie et bilans, gardés sur ce PC.' },
  { id: 'wikelo', label: 'Wikelo', icon: 'star', barre: true, eyebrow: 'Activités', titre: 'Contrats de Wikelo', desc: 'Les contrats de Wikelo en français, avec les objets à réunir.' },
  { id: 'personnages', label: 'Personnages', icon: 'user', barre: true, eyebrow: 'Apparences', titre: 'Personnages', desc: 'Tes apparences du jeu sauvegardées sur le portail, avec la photo du visage.' },
  { id: 'outils', label: 'Réglages', icon: 'settings', barre: true, eyebrow: 'CES Compagnon', titre: 'Réglages', desc: 'Interface, fond, notifications, raccourcis, mises à jour et partages.' },
  { id: 'notifications', label: 'Notifications', icon: 'bell', eyebrow: 'Portail CES', titre: 'Notifications', desc: 'Ce que le portail t’a signalé : opérations, SOS, DEFCON, forum, invitations.' },
  { id: 'partage', label: 'Partage', icon: 'shield', eyebrow: 'Confidentialité', titre: 'Partage', desc: 'Ce que l’application envoie au portail, type par type.' },
  { id: 'compte', label: 'Compte', icon: 'user', eyebrow: 'Identité', titre: 'Compte', desc: 'Ton compte CES relié à cet appareil.' },
  { id: 'apropos', label: 'À propos', icon: 'info', eyebrow: 'CES Compagnon', titre: 'À propos', desc: 'Version, canal, licences, sources de données et mentions.' },
];

// Présence (contrat d'API § 3) → libellé et pastille Aurore (DS Status).
const PRESENCE = {
  offline: { label: 'Star Citizen n’est pas lancé', state: 'offline' },
  launching: { label: 'Lancement de Star Citizen…', state: 'idle' },
  menu: { label: 'Au menu principal', state: 'idle' },
  universe: { label: 'Dans l’univers', state: 'online' },
};

const REGIONS = { euw: 'Europe de l’Ouest', eu: 'Europe', use: 'Est des États-Unis', usw: 'Ouest des États-Unis', apse: 'Asie-Pacifique' };

const heure = (iso) => new Date(iso).toLocaleString('fr-FR', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' });
function duree(depuisIso) {
  const min = Math.max(0, Math.round((Date.now() - new Date(depuisIso)) / 60000));
  return min < 60 ? `${min} min` : `${Math.floor(min / 60)} h ${String(min % 60).padStart(2, '0')}`;
}

/** Libellé lisible d'un événement (types du contrat § 5). Les noms de vaisseaux sont les noms internes du jeu. */
function decrire(e) {
  switch (e.type) {
    case 'session.start': return { icon: 'rocket', texte: 'Session de jeu commencée' };
    case 'session.end': return { icon: 'close', texte: 'Session de jeu terminée' };
    case 'presence': return { icon: 'pin', texte: `Présence : ${PRESENCE[e.state]?.label ?? e.state}${e.region ? ` (${REGIONS[e.region] ?? e.region})` : ''}` };
    case 'vehicle.destroyed': {
      const qui = { owner: 'Ton vaisseau détruit', killer: 'Vaisseau détruit par toi', both: 'Ton vaisseau détruit par toi' }[e.role];
      const pourquoi = { combat: 'combat', collision: 'collision', other: 'autre' }[e.reason];
      return { icon: 'sword', texte: `${qui} : ${e.vehicleClass} (${pourquoi})` };
    }
    case 'player.down': return { icon: 'medic', texte: e.kind === 'dead' ? 'Mort (réveil en lit médical)' : 'À terre (incapacité)' };
    case 'travel.quantum': return { icon: 'ship', texte: `Saut quantique terminé : ${e.vehicleClass}` };
    case 'mission.completed': return { icon: 'flag', texte: 'Mission réussie' };
    default: return { icon: 'info', texte: e.type };
  }
}

/** État du compte relié et de l'envoi de la présence (cœur Rust, commande `compte_etat`). */
function useCompte() {
  const [compte, setCompte] = useState(null);
  const relire = useCallback(() => invoke('compte_etat').then(setCompte).catch(() => {}), []);
  useEffect(() => {
    relire();
    const t = setInterval(relire, 3000);
    return () => clearInterval(t);
  }, [relire]);
  return { compte, relire };
}

/** Notifications non lues du portail (cloche de la barre de titre), relues toutes les 45 s une fois connecté. */
function useNotifications(relie) {
  const [nonLues, setNonLues] = useState(null);
  useEffect(() => {
    if (!relie) { setNonLues(null); return undefined; }
    const lire = () => invoke('notifications_non_lues').then(setNonLues).catch(() => {});
    lire();
    const t = setInterval(lire, 45000);
    return () => clearInterval(t);
  }, [relie]);
  return { nonLues, fixer: setNonLues, relire: () => invoke('notifications_non_lues').then(setNonLues).catch(() => {}) };
}

/** Tiroir des notifications NATIF à l'application (pages locales comme portail), même style que le portail. */

/** Signale l'ouverture d'un Popover (le menu du compte) : la webview du site est masquée pendant ce temps. */
function SignalOuvert({ open, onEtat, children }) {
  useEffect(() => { onEtat(open); }, [open, onEtat]);
  return children;
}

/** Menu du compte (Partage, Compte, À propos), accessible partout, y compris au-dessus du site embarqué. */
function MenuCompte({ libelle, entrees, onEtat, actif }) {
  return (
    <Popover align="start" trigger={({ open, toggle }) => (
      <SignalOuvert open={open} onEtat={onEtat}>
        <button type="button" className={actif || open ? 'hud-rail__lien is-actif' : 'hud-rail__lien'} aria-haspopup="menu" aria-expanded={open} onClick={toggle} title={libelle}>
          <span className="hud-rail__avatar">{(libelle || '?').slice(0, 1).toUpperCase()}</span>
          <span className="hud-rail__txt">Compte</span>
        </button>
      </SignalOuvert>
    )}>
      {({ close }) => (
        <div role="menu" className="ds-menu cmp-menu">
          {entrees.map((e) => (
            <button key={e.label} type="button" role="menuitem" className="ds-menu__item" onClick={() => { e.onSelect(); close(); }}>
              <Icon name={e.icon} size={16} /><span>{e.label}</span>
            </button>
          ))}
        </div>
      )}
    </Popover>
  );
}

// Visite guidée (demande du CES, 0.14.0) : un projecteur qui glisse d'un élément à l'autre, une bulle animée.
// `cible` = attribut data-visite de l'élément ; `ecran` = écran à afficher avant de le chercher ; sans cible = étape centrée.
const VISITE = [
  { titre: 'Bienvenue dans CES Compagnon', texte: 'En une minute, le tour de l’application : tes outils de jeu, le portail du CES et les alertes de l’escadre.', logo: true },
  { cible: 'nav-app', ecran: 'accueil', titre: 'Tes outils', texte: 'Accueil, Carnet de voyage, Activités (commerce, minage, Wikelo) et Outils (traduction, touches) : tout ce qui tourne sur ton PC.' },
  { cible: 'nav-portail', titre: 'Le portail du CES', texte: 'Missions, forum, annuaire, DEFCON et tes données : directement dans l’application, sans te reconnecter.' },
  { cible: 'cloche', titre: 'Notifications', texte: 'Les notifications du portail arrivent ici et dans Windows. Tu peux les mettre en sourdine dans Outils.' },
  { cible: 'compte', titre: 'Ton compte', texte: 'En bas du rail : Partage (ce que l’application envoie au portail), Compte, À propos, et cette visite si tu veux la revoir.' },
  { cible: 'sos', titre: 'SOS et RSI Launcher', texte: 'En difficulté en jeu ? Maintiens le bouton SOS une seconde pour appeler le CES. À côté, « Launcher » ouvre le RSI Launcher.' },
  { cible: 'jeu', ecran: 'accueil', titre: 'Ton poste de bord', texte: 'Des fenêtres que tu ranges à ta façon : glisse-les par leur titre, agrandis-les par leur coin. L’application voit quand Star Citizen tourne, sans jamais toucher au jeu.' },
  { cible: 'carte', ecran: 'carnet', titre: 'Ta carte stellaire', texte: 'Tes trajets entre Stanton, Pyro et Nyx, tirés de tes sessions. Fais-la tourner, zoome, clique sur un système.' },
  { cible: 'notifs', ecran: 'outils', titre: 'Alertes et Discord', texte: 'Lancements des membres, alertes DEFCON, notifications du portail et présence Discord : tout se règle ici.' },
  { titre: 'Bon vol, pilote !', texte: 'Tu peux relancer cette visite à tout moment depuis le menu de ton compte, en bas du rail.', logo: true },
];

function VisiteGuidee({ ouverte, onFermer, allerEcran }) {
  const [i, setI] = useState(0);
  const [rect, setRect] = useState(null);
  const etape = VISITE[i];
  useEffect(() => { if (ouverte) setI(0); }, [ouverte]);
  useEffect(() => {
    if (!ouverte) return undefined;
    if (etape.ecran) allerEcran(etape.ecran);
    let actif = true;
    const mesurer = () => {
      const el = etape.cible && document.querySelector(`[data-visite="${etape.cible}"]`);
      if (!actif) return;
      if (etape.cible && !el) { setI((n) => Math.min(n + 1, VISITE.length - 1)); return; } // élément absent : étape suivante
      if (el) el.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      setTimeout(() => {
        if (!actif) return;
        const r = el?.getBoundingClientRect();
        setRect(r ? { x: r.left - 8, y: r.top - 8, l: r.width + 16, h: r.height + 16 } : null);
      }, el ? 260 : 0);
    };
    const t = setTimeout(mesurer, etape.ecran ? 320 : 40);
    window.addEventListener('resize', mesurer);
    return () => { actif = false; clearTimeout(t); window.removeEventListener('resize', mesurer); };
  }, [ouverte, i]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (!ouverte) return undefined;
    const clavier = (e) => {
      if (e.key === 'Escape') onFermer();
      if (e.key === 'ArrowRight' || e.key === 'Enter') setI((n) => (n + 1 < VISITE.length ? n + 1 : n));
      if (e.key === 'ArrowLeft') setI((n) => Math.max(0, n - 1));
    };
    window.addEventListener('keydown', clavier);
    return () => window.removeEventListener('keydown', clavier);
  }, [ouverte, onFermer]);
  if (!ouverte) return null;
  const derniere = i === VISITE.length - 1;
  // Bulle sous la cible si la place le permet, sinon au-dessus ; centrée sans cible.
  const W = window.innerWidth; const H = window.innerHeight;
  const bulle = rect
    ? { left: Math.min(Math.max(16, rect.x), W - 376), top: rect.y + rect.h + 14 + 220 < H ? rect.y + rect.h + 14 : Math.max(16, rect.y - 14 - 210) }
    : { left: W / 2 - 180, top: H / 2 - 150 };
  return (
    <div className="cmp-visite" role="dialog" aria-modal="true" aria-label="Visite guidée">
      <div className={rect ? 'cmp-visite__projecteur' : 'cmp-visite__projecteur is-plein'}
        style={rect ? { transform: `translate(${rect.x}px, ${rect.y}px)`, width: rect.l, height: rect.h } : undefined} />
      <div key={i} className="cmp-visite__bulle" style={bulle}>
        {etape.logo && <LogoCompagnon className="cmp-visite__logo" />}
        <div className="cmp-visite__etape">Étape {i + 1} sur {VISITE.length}</div>
        <h3>{etape.titre}</h3>
        <p>{etape.texte}</p>
        <div className="cmp-visite__points" aria-hidden="true">{VISITE.map((_, n) => <i key={n} className={n === i ? 'is-actif' : n < i ? 'is-fait' : ''} />)}</div>
        <div className="cmp-visite__actions">
          <Button variant="text" severity="secondary" size="sm" label={derniere ? 'Fermer' : 'Passer'} onClick={onFermer} />
          <span style={{ flex: 1 }} />
          {i > 0 && <Button variant="outlined" size="sm" icon="left" label="Précédent" onClick={() => setI(i - 1)} />}
          <Button variant="aurora" size="sm" label={derniere ? 'C’est parti' : 'Suivant'} iconRight={derniere ? undefined : 'right'} onClick={() => (derniere ? onFermer() : setI(i + 1))} />
        </div>
      </div>
    </div>
  );
}

function useEtat() {
  const [etat, setEtat] = useState(null);
  const [erreur, setErreur] = useState(null);
  useEffect(() => {
    let actif = true;
    const lire = () =>
      invoke('etat')
        .then((e) => { if (actif) { setEtat(e); setErreur(null); } })
        .catch((e) => { if (actif) setErreur(String(e)); });
    lire();
    const t = setInterval(lire, 3000);
    return () => { actif = false; clearInterval(t); };
  }, []);
  return { etat, erreur };
}

function PanneauPortail({ compte, allerCompte }) {
  if (!compte) return null;
  if (!compte.relie) {
    return (
      <Panel header="Portail CES">
        {compte.erreur && <Message severity="warn">{compte.erreur}</Message>}
        <p className="cmp-muted">Compte non relié : ta présence n’est pas partagée.</p>
        <div className="cmp-actions"><Button label="Relier mon compte" icon="link" onClick={allerCompte} /></div>
      </Panel>
    );
  }
  const u = compte.utilisateur;
  return (
    <Panel header="Portail CES">
      <dl className="cmp-dl">
        <dt>Compte</dt><dd>{u?.username}{u?.matricule ? ` · ${u.matricule}` : ''}</dd>
        <dt>Présence</dt>
        <dd>
          {compte.partagePresence === false ? 'non partagée (réglage sur le portail)'
            : compte.etatEnvoye ? `${PRESENCE[compte.etatEnvoye]?.label ?? compte.etatEnvoye} — envoyée à ${heure(compte.dernierEnvoi)}`
            : 'partagée dès que le jeu est lancé'}
        </dd>
        {compte.operation && (<><dt>Opération</dt><dd>{compte.operation.title}</dd></>)}
      </dl>
      {compte.erreur && <Message severity="warn">{compte.erreur}</Message>}
    </Panel>
  );
}

/** Choix manuel du dossier du jeu (vérifié : LIVE / PTU), ou retour à la détection automatique. */
function ChoixDossier({ manuel }) {
  const [msg, setMsg] = useState(null);
  const choisir = async () => {
    try {
      const d = await invoke('jeu_choisir_dossier');
      if (d) setMsg({ severity: 'success', texte: `Dossier retenu : ${d}` });
    } catch (e) { setMsg({ severity: 'danger', texte: String(e) }); }
  };
  const auto = async () => { await invoke('jeu_dossier_auto').catch(() => {}); setMsg({ severity: 'info', texte: 'Détection automatique rétablie.' }); };
  return (
    <>
      <div className="cmp-actions">
        <Button size="sm" variant="outlined" icon="folder" label="Choisir le dossier du jeu…" onClick={choisir} />
        {manuel && <Button size="sm" variant="text" label="Détection automatique" onClick={auto} />}
      </div>
      {msg && <div className="cmp-gap"><Message severity={msg.severity} onClose={() => setMsg(null)}>{msg.texte}</Message></div>}
    </>
  );
}

function PanneauJeu({ etat }) {
  const p = PRESENCE[etat.presence] ?? PRESENCE.offline;
  const enJeu = etat.presence !== 'offline';
  return (
      <div data-visite="jeu" className="cmp-visite-cible"><Panel header="Le jeu">
        <div className="cmp-row">
          <Status label={p.label} state={p.state} />
        </div>
        <dl className="cmp-dl">
          {enJeu && etat.sessionDebut && (<><dt>Session</dt><dd>depuis {duree(etat.sessionDebut)}</dd></>)}
          {etat.region && (<><dt>Région</dt><dd>{REGIONS[etat.region] ?? etat.region}</dd></>)}
          {etat.version && (<><dt>Version du jeu</dt><dd className="cmp-mono">{etat.version}</dd></>)}
          <dt>Dossier du jeu</dt>
          <dd className="cmp-mono">{etat.dossierJeu ?? 'introuvable'}</dd>
          <dt>Journal</dt>
          <dd>{etat.journalTrouve ? `trouvé (${etat.journalTailleMo ? `${etat.journalTailleMo} Mo` : 'moins de 1 Mo'})` : 'introuvable'}</dd>
        </dl>
        {!etat.dossierJeu && (
          <Message severity="warn" title="Dossier de Star Citizen introuvable">
            Indique-le à la main : le dossier LIVE (ou PTU), qui contient Bin64 et Data.p4k.
          </Message>
        )}
        <ChoixDossier manuel={etat.dossierManuel} />
      </Panel></div>
  );
}


// Accueil (24/11, le CES : « un mélange entre passerelle et tuiles », traduction et touches sur l'accueil) : bandeau de
// lectures fixe, puis des tuiles déplaçables. Grille de 12 colonnes, rangs de 34 px.
const TUILES_ACCUEIL = [
  { id: 'operation', titre: 'Prochaine opération', defaut: { x: 0, y: 0, w: 8, h: 8 } },
  { id: 'alerte', titre: 'Alerte et SOS', defaut: { x: 8, y: 0, w: 4, h: 8 } },
  { id: 'enjeu', titre: 'En jeu maintenant', defaut: { x: 0, y: 8, w: 4, h: 6 } },
  { id: 'cesoir', titre: 'Ce soir', defaut: { x: 4, y: 8, w: 4, h: 6 } },
  { id: 'radio', titre: 'Radio du CES', defaut: { x: 8, y: 8, w: 4, h: 4 } },
  { id: 'raccourcis', titre: 'Raccourcis', defaut: { x: 8, y: 12, w: 4, h: 6 } },
  { id: 'traduction', titre: 'Traduction du jeu', defaut: { x: 0, y: 14, w: 6, h: 10 } },
  { id: 'jeu', titre: 'Le jeu', defaut: { x: 6, y: 18, w: 6, h: 8 } },
  { id: 'touches', titre: 'Touches du jeu', defaut: { x: 0, y: 24, w: 6, h: 10 } },
  { id: 'portail', titre: 'Portail CES', defaut: { x: 6, y: 26, w: 6, h: 5 }, cachee: true },
  { id: 'statut', titre: 'Services CES', defaut: { x: 6, y: 31, w: 6, h: 7 }, cachee: true },
];

function Accueil({ etat, erreur, compte, allerCompte, sos, ouvrirSos, defcon, aller, ouvrirRadio }) {
  if (erreur) return <Message severity="danger" title="Le cœur de l’application ne répond pas">{erreur}</Message>;
  if (!etat) return <p className="cmp-muted">Chargement…</p>;
  const jeuLance = !!etat?.jeuLance;
  const rendus = {
    operation: () => <BlocOperation aller={aller} moiId={compte?.moi?.id} />,
    alerte: () => <BlocAlerte defcon={defcon} sos={sos} aller={aller} ouvrirSos={ouvrirSos} />,
    enjeu: () => <BlocEnJeu />,
    cesoir: () => <BlocCeSoir aller={aller} />,
    radio: () => <BlocRadio ouvrirRadio={ouvrirRadio} />,
    raccourcis: () => <BlocOutils aller={aller} />,
    traduction: () => <Traduction jeuLance={jeuLance} />,
    touches: () => <Touches jeuLance={jeuLance} />,
    jeu: () => <PanneauJeu etat={etat} />,
    portail: () => <PanneauPortail compte={compte} allerCompte={allerCompte} />,
    statut: () => <StatutCes />,
  };
  return (
    <div className="psl-page">
      <BandeauPasserelle etat={etat} defcon={defcon} sos={sos} />
      <TableauDeBord cle="cmp-tdb-accueil-v2" tuiles={TUILES_ACCUEIL.map((t) => ({ ...t, rendu: rendus[t.id] }))} />
    </div>
  );
}

/** Vérifie une mise à jour au démarrage puis toutes les 6 h ; ne dit rien s'il n'y en a pas ou en cas d'erreur
 *  (hors ligne, dépôt pas encore créé…). Installation seulement sur clic du membre. */
/** Mise à jour : vérifiée au démarrage puis toutes les 6 h (silence hors ligne). Un rappel discret sur les pages, et une
 *  fenêtre dans le style de l'application (versions, notes, progression réelle). Installation seulement sur clic. */
function MiseAJour() {
  const [maj, setMaj] = useState(null);
  const [ouverte, setOuverte] = useState(false);
  const [etape, setEtape] = useState(null); // null | 'telechargement' | 'installation' | 'erreur'
  const [prog, setProg] = useState(null); // { recu, total }
  useEffect(() => {
    let actif = true;
    const verifier = () => invoke('maj_verifier').then((u) => { if (actif && u) setMaj(u); }).catch(() => {});
    verifier();
    const t = setInterval(verifier, 6 * 3600 * 1000);
    // Bouton « Vérifier les mises à jour » des Réglages : ouvre directement la fenêtre si une version est trouvée.
    const trouvee = (e) => { if (actif && e.detail) { setMaj(e.detail); setOuverte(true); } };
    window.addEventListener('cmp-maj-trouvee', trouvee);
    return () => { actif = false; clearInterval(t); window.removeEventListener('cmp-maj-trouvee', trouvee); };
  }, []);
  if (!maj) return null;
  const installer = async () => {
    setEtape('telechargement');
    const canal = new Channel();
    canal.onmessage = (m) => { if (m.etape) setEtape(m.etape); else setProg(m); };
    try {
      await invoke('maj_installer', { progression: canal }); // installe puis relance l'application
    } catch {
      setEtape('erreur');
    }
  };
  const pct = prog?.total ? Math.min(100, Math.round((prog.recu / prog.total) * 100)) : null;
  const mo = (o) => `${(o / 1048576).toFixed(1)} Mo`;
  const occupe = etape === 'telechargement' || etape === 'installation';
  return (
    <>
      <Message severity="info" title={`CES Compagnon ${maj.version} est disponible`}>
        <div className="cmp-actions"><Button size="sm" label="Voir la mise à jour" onClick={() => setOuverte(true)} /></div>
      </Message>
      <Dialog open={ouverte} onClose={() => { if (!occupe) setOuverte(false); }} title="Mise à jour"
        footer={(
          <>
            <Button variant="text" severity="secondary" label="Plus tard" disabled={occupe} onClick={() => setOuverte(false)} />
            <Button variant="aurora" label={etape === 'erreur' ? 'Réessayer' : 'Installer et redémarrer'} loading={occupe} onClick={installer} />
          </>
        )}>
        <div className="cmp-maj">
          <LogoCompagnon className="cmp-maj__logo" />
          <p className="cmp-maj__versions"><span className="cmp-mono">{__APP_VERSION__}</span> <Icon name="right" size={14} /> <strong className="cmp-mono">{maj.version}</strong></p>
          {maj.notes && <div className="cmp-maj__notes">{maj.notes}</div>}
          {occupe && (
            <div className="cmp-maj__prog">
              <ProgressBar aurora value={etape === 'installation' ? 100 : pct ?? 0} indeterminate={etape === 'telechargement' && pct == null} label="Progression de la mise à jour" />
              <p className="cmp-muted cmp-small">
                {etape === 'installation' ? 'Installation… l’application va redémarrer.'
                  : prog?.total ? `Téléchargement : ${mo(prog.recu)} / ${mo(prog.total)} (${pct} %)` : 'Téléchargement…'}
              </p>
            </div>
          )}
          {etape === 'erreur' && <Message severity="danger">La mise à jour a échoué (connexion ?). Réessaie plus tard.</Message>}
          <p className="cmp-muted cmp-small">Paquet signé : sa signature est vérifiée avant l’installation.</p>
        </div>
      </Dialog>
    </>
  );
}

function Journal({ etat }) {
  const liste = (etat?.evenements ?? []).filter((e) => e.type !== 'presence');
  if (!liste.length) {
    return <EmptyState icon="file" title="Rien pour l’instant" text="Les événements de tes sessions de jeu apparaîtront ici." />;
  }
  return (
    <div className="cmp-stack">
      <Message severity="info" title="Ce qui part au portail">
        Seuls les types activés dans « Partage », survenus après l’appairage, sont envoyés. Les autres restent sur ce PC.
      </Message>
      <ul className="cmp-events">
        {liste.map((e) => {
          const d = decrire(e);
          return (
            <li key={`${e.at}-${e.decalage}-${e.type}`} className={e.historique ? 'is-old' : undefined}>
              <Icon name={d.icon} size={16} />
              <span className="cmp-events__txt">{d.texte}</span>
              <time className="cmp-mono">{heure(e.at)}</time>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

const NOM_DEFAUT = 'PC Windows';

/** Écran Compte : appairage par code (contrat § 2), puis compte relié et déconnexion. */
function Compte({ compte, relire, porte = false }) {
  const [nom, setNom] = useState(NOM_DEFAUT);
  const [lien, setLien] = useState(null);       // { linkId, code, verifyUrl, expiresIn, interval }
  const [reste, setReste] = useState(0);        // secondes avant expiration du code
  const [occupe, setOccupe] = useState(false);
  const [message, setMessage] = useState(null); // { severity, texte }

  // Sondage du résultat pendant que le code est affiché.
  useEffect(() => {
    if (!lien) return undefined;
    let actif = true;
    const fin = Date.now() + lien.expiresIn * 1000;
    setReste(lien.expiresIn);
    const minuteur = setInterval(() => setReste(Math.max(0, Math.round((fin - Date.now()) / 1000))), 1000);
    const sonde = setInterval(async () => {
      const r = await invoke('appairage_suivi', { linkId: lien.linkId, nom }).catch((e) => ({ status: 'error', message: String(e) }));
      if (!actif || r.status === 'pending') return;
      setLien(null);
      if (r.status === 'approved') {
        setMessage({ severity: 'success', texte: 'Compte relié. Ta présence sera partagée selon tes réglages.' });
        relire();
      } else {
        setMessage({ severity: r.status === 'denied' ? 'warn' : 'danger', texte: r.message });
      }
    }, Math.max(2, lien.interval) * 1000);
    return () => { actif = false; clearInterval(minuteur); clearInterval(sonde); };
  }, [lien, nom, relire]);

  const commencer = async () => {
    setOccupe(true);
    setMessage(null);
    try { setLien(await invoke('appairage_debut', { nom })); }
    catch (e) { setMessage({ severity: 'danger', texte: String(e) }); }
    finally { setOccupe(false); }
  };
  const delier = async () => {
    setOccupe(true);
    try { await invoke('deconnexion'); setMessage({ severity: 'info', texte: 'Cet appareil n’est plus relié.' }); }
    catch (e) { setMessage({ severity: 'warn', texte: String(e) }); }
    finally { setOccupe(false); relire(); }
  };

  if (!compte) return <p className="cmp-muted">Chargement…</p>;
  const retour = message && <Message severity={message.severity}>{message.texte}</Message>;

  if (compte.relie) {
    const u = compte.utilisateur;
    return (
      <div className="cmp-stack">
        {retour}
        <Panel header="Compte relié">
          <dl className="cmp-dl">
            <dt>Membre</dt><dd>{u?.username}</dd>
            {u?.matricule && (<><dt>Matricule</dt><dd className="cmp-mono">{u.matricule}</dd></>)}
            <dt>Appareil</dt><dd>{compte.appareil}</dd>
            <dt>Portail</dt><dd>{__CES_PORTAIL__.replace('https://', '')}</dd>
          </dl>
          <p className="cmp-muted cmp-gap">Ce que l’application partage se règle sur le portail : Profil › Modifier › Sécurité › CES Compagnon.</p>
          <div className="cmp-actions">
            <Button label="Se déconnecter" severity="danger" variant="outlined" icon="close" loading={occupe} onClick={delier} />
          </div>
        </Panel>
      </div>
    );
  }

  if (lien) {
    return (
      <div className="cmp-stack">
        <Panel header="Approuve ce code sur le portail">
          <p className="cmp-muted">Une page du portail s’est ouverte dans ton navigateur. Vérifie que le code est le même, puis approuve.</p>
          <div className="cmp-code cmp-mono" aria-live="polite">{lien.code}</div>
          <p className="cmp-muted">Expire dans {Math.floor(reste / 60)} min {String(reste % 60).padStart(2, '0')} s · en attente de ton approbation…</p>
          <div className="cmp-actions">
            <Button label="Rouvrir la page" variant="outlined" icon="link" onClick={() => invoke('ouvrir_page', { url: lien.verifyUrl }).catch(() => {})} />
            <Button label="Annuler" variant="text" onClick={() => setLien(null)} />
          </div>
        </Panel>
      </div>
    );
  }

  return (
    <div className="cmp-stack">
      {retour || (compte.erreur && <Message severity="warn">{compte.erreur}</Message>)}
      <Panel header={porte ? 'Se connecter' : 'Relier mon compte CES'}>
        <p className="cmp-muted">
          L’application affiche un code et ouvre le portail dans ton navigateur : tu t’y connectes comme d’habitude
          (Discord ou clé d’accès) et tu approuves. Aucun mot de passe n’est saisi ici.
        </p>
        <div className="cmp-gap">
          <Field label="Nom de cet appareil" htmlFor="cmp-nom" help="Visible dans ton profil, pour le reconnaître et le délier.">
            <InputText id="cmp-nom" value={nom} maxLength={60} onChange={(e) => setNom(e.target.value)} />
          </Field>
        </div>
        <div className="cmp-actions">
          <Button label={porte ? 'Se connecter' : 'Relier mon compte'} variant="aurora" icon="link" loading={occupe} onClick={commencer} />
        </div>
      </Panel>
    </div>
  );
}

// Réglages de partage (contrat § 4) : ce que chaque type envoie, en clair.
const REGLAGES = [
  { cle: 'presence', titre: 'Statut « en jeu »', desc: 'Les membres voient que tu joues (menu ou univers, région du serveur). Jamais ton adresse IP.' },
  { cle: 'announceLaunch', titre: 'Annonce de lancement', desc: '« kalgio a lancé Star Citizen » aux membres connectés, au plus une fois par heure.' },
  { cle: 'session', titre: 'Sessions de jeu', desc: 'Début, fin et durée de tes sessions (temps de jeu sur ton profil).' },
  { cle: 'vehicle', titre: 'Vaisseaux détruits', desc: 'Les tiens et ceux que tu détruis : modèle, cause. Jamais le nom de l’autre joueur.' },
  { cle: 'down', titre: 'Morts et incapacités', desc: 'Quand tu tombes, pour le débrief des opérations.' },
  { cle: 'travel', titre: 'Sauts quantiques et lieux visités', desc: 'Fin de chaque saut (vaisseau, destination), lieux et juridictions traversés — jamais les balises de mission.' },
  { cle: 'missions', titre: 'Missions réussies', desc: 'Chaque contrat terminé avec succès.' },
  { cle: 'mining', titre: 'Sessions de minage', desc: 'Durée, minerais (SCU), raffinerie et gains de tes sessions de minage, quand tu les enregistres.' },
];

function Partage({ compte, allerCompte }) {
  const [reglages, setReglages] = useState(null);
  const [encours, setEncours] = useState(null);
  const [erreur, setErreur] = useState(null);
  const relie = compte?.relie;

  useEffect(() => {
    if (!relie) return;
    invoke('reglages_lire').then(setReglages).catch((e) => setErreur(String(e)));
  }, [relie]);

  if (!compte) return <p className="cmp-muted">Chargement…</p>;
  if (!relie) {
    return (
      <EmptyState icon="shield" title="Compte non relié" text="Relie ton compte CES pour choisir ce que l’application partage."
        action={<Button label="Relier mon compte" icon="link" onClick={allerCompte} />} />
    );
  }

  const basculer = async (cle, valeur) => {
    setEncours(cle);
    setErreur(null);
    try { setReglages(await invoke('reglages_ecrire', { partiel: { [cle]: valeur } })); }
    catch (e) { setErreur(String(e)); }
    finally { setEncours(null); }
  };

  return (
    <div className="cmp-stack">
      {erreur && <Message severity="danger">{erreur}</Message>}
      <Panel header="Ce que l’application partage avec le portail">
        <p className="cmp-muted">
          Uniquement tes propres événements, jamais le journal du jeu ni d’informations sur les autres joueurs.
          Un type désactivé reste visible ici, dans le Journal, mais ne quitte pas ce PC.
        </p>
        {!reglages ? <p className="cmp-muted cmp-gap">Chargement des réglages…</p> : (
          <div className="cmp-reglages">
            {REGLAGES.map((r) => (
              <SettingRow key={r.cle} title={r.titre} desc={r.desc} htmlFor={`rg-${r.cle}`}>
                <ToggleSwitch id={`rg-${r.cle}`} checked={reglages[r.cle] === true} disabled={encours === r.cle}
                  onChange={(v) => basculer(r.cle, v)} />
              </SettingRow>
            ))}
          </div>
        )}
        <p className="cmp-muted cmp-gap">
          Tu peux relire et effacer ce qui a été partagé sur le portail (Profil › Modifier › Sécurité › CES Compagnon).
        </p>
      </Panel>
      {compte.fileAttente > 0 && (
        <Message severity="info">{compte.fileAttente} événement(s) en attente d’envoi (connexion au portail).</Message>
      )}
    </div>
  );
}

const SOURCES = {
  scefra: { nom: 'SCEFRA (SPEED0U)', credit: 'Traduction française par SPEED0U / SCEFRA — CC BY-NC-ND 4.0 — github.com/SPEED0U/Scefra. Fichier téléchargé chez SCEFRA et adapté sur ce PC au format que lit le jeu (marque UTF-8, en-tête en commentaires « ; ») ; message d’accueil et page du journal sur Multitool remplacés par ceux du CES, avec le crédit SCEFRA ; aucune copie modifiée n’est publiée.' },
  circuspes: { nom: 'Circuspes', credit: 'Traduction française Circuspes — traduction.circuspes.fr' },
  inconnue: { nom: 'non reconnue', credit: null },
};
const jour = (iso) => new Date(iso).toLocaleDateString('fr-FR', { day: 'numeric', month: 'long', year: 'numeric' });
const mo = (o) => `${(o / 1048576).toFixed(1).replace('.', ',')} Mo`;
const dateSauvegarde = (d) => heure(d.replace(/^(\d{4}-\d\d-\d\d)T(\d\d)-(\d\d)-(\d\d)-(\d{3})Z$/, '$1T$2:$3:$4.$5Z'));

/** Fenêtre de confirmation (DS Dialog). */
function Confirmer({ ouvert, titre, children, libelle, onOui, onNon, occupe }) {
  return (
    <Dialog open={ouvert} onClose={onNon} title={titre}
      footer={<><Button label="Annuler" variant="text" onClick={onNon} /><Button label={libelle} loading={occupe} onClick={onOui} /></>}>
      {children}
    </Dialog>
  );
}

function Traduction({ jeuLance }) {
  const [t, setT] = useState(null);
  const [nbSauv, setNbSauv] = useState(0);
  const [occupe, setOccupe] = useState(false);
  const [confirmer, setConfirmer] = useState(null); // 'installer' | 'restaurer'
  const [retour, setRetour] = useState(null);
  const lire = useCallback(async (forcer = false) => {
    setOccupe(true);
    try {
      setT(await invoke('traduction_etat', { forcer }));
      setNbSauv(await invoke('traduction_sauvegardes'));
    } catch { /* garde l'état précédent */ } finally { setOccupe(false); }
  }, []);
  useEffect(() => { lire(); }, [lire]);

  const agir = async (commande) => {
    setOccupe(true);
    setRetour(null);
    try { setRetour({ severity: 'success', texte: await invoke(commande) }); }
    catch (e) { setRetour({ severity: 'danger', texte: String(e) }); }
    finally { setConfirmer(null); setOccupe(false); lire(); }
  };

  if (!t) return <Panel header="Traduction du jeu"><p className="cmp-muted">Analyse du fichier de traduction…</p></Panel>;
  const src = SOURCES[t.source] ?? SOURCES.inconnue;
  const d = t.distant;
  const francais = (t.langue ?? '').toLowerCase().startsWith('french');
  const peutInstaller = !t.present || d?.aJour === false || !francais || t.aAdapter;
  let verdict = null;
  if (!t.langue) verdict = <Message severity="warn">Langue du jeu introuvable (fichier user.cfg).</Message>;
  else if (!francais) verdict = <Message severity="info">Le jeu n’est pas réglé en français. L’installation règle aussi la langue.</Message>;
  else if (!t.present) verdict = <Message severity="warn" title="Fichier de traduction absent">Le jeu est réglé en français mais global.ini est introuvable : les textes s’afficheront en clés brutes.</Message>;
  else if (t.aAdapter) verdict = <Message severity="warn" title="Traduction à réinstaller">Ce fichier SCEFRA n’est pas encore adapté par CES Compagnon (format que lit le jeu, message d’accueil du CES). Réinstalle-le : les traductions ne changent pas.</Message>;
  else if (d?.erreur) verdict = <Message severity="warn">{d.erreur}</Message>;
  else if (d?.aJour === true) verdict = <Message severity="success" title="Traduction à jour">Identique à la dernière version de SCEFRA.</Message>;
  else if (d?.aJour === false) verdict = <Message severity="warn" title="Une version plus récente existe">SCEFRA a publié une mise à jour{d.date ? ` le ${jour(d.date)}` : ''}{d.taille ? ` (${mo(d.taille)})` : ''}.</Message>;

  return (
    <Panel header="Traduction du jeu">
      <dl className="cmp-dl">
        <dt>Langue du jeu</dt><dd>{t.langue ?? '—'}{t.langueAudio ? ` (voix : ${t.langueAudio})` : ''}</dd>
        {t.present && (<>
          <dt>Source</dt><dd>{src.nom}{t.licence ? ` · ${t.licence}` : ''}</dd>
          <dt>Fichier installé</dt><dd>{t.modifie ? jour(t.modifie) : '—'} · {mo(t.taille)} · {t.entrees?.toLocaleString('fr-FR')} textes</dd>
        </>)}
        {d?.verifieA && (<><dt>Vérifié</dt><dd>{heure(d.verifieA)}</dd></>)}
      </dl>
      {verdict && <div className="cmp-gap">{verdict}</div>}
      {retour && <div className="cmp-gap"><Message severity={retour.severity}>{retour.texte}</Message></div>}
      <div className="cmp-actions">
        {peutInstaller && <Button label="Installer la traduction SCEFRA" icon="download" disabled={jeuLance} loading={occupe && confirmer === 'installer'} onClick={() => setConfirmer('installer')} />}
        <Button label="Vérifier" variant="outlined" icon="search" loading={occupe && !confirmer} onClick={() => lire(true)} />
        {nbSauv > 0 && <Button label="Restaurer la précédente" variant="text" icon="undo" disabled={jeuLance} onClick={() => setConfirmer('restaurer')} />}
      </div>
      {jeuLance && <p className="cmp-muted cmp-gap">Ferme Star Citizen pour installer ou restaurer la traduction.</p>}
      {src.credit && <p className="cmp-muted cmp-gap cmp-small">{src.credit}</p>}

      <Confirmer ouvert={confirmer === 'installer'} titre="Installer la traduction SCEFRA ?" libelle="Installer" occupe={occupe}
        onOui={() => agir('traduction_installer')} onNon={() => setConfirmer(null)}>
        <p>Le fichier officiel de SCEFRA (environ 12 Mo) sera téléchargé, vérifié, adapté au format que lit le jeu (marque UTF-8, en-tête de licence en commentaires « ; »), avec le message d’accueil Multitool remplacé par celui du CES (crédit SCEFRA conservé), puis installé dans le dossier du jeu.</p>
        <p>Ton fichier actuel est d’abord <strong>sauvegardé</strong> : tu peux revenir en arrière avec « Restaurer la précédente ».</p>
      </Confirmer>
      <Confirmer ouvert={confirmer === 'restaurer'} titre="Restaurer la traduction précédente ?" libelle="Restaurer" occupe={occupe}
        onOui={() => agir('traduction_restaurer')} onNon={() => setConfirmer(null)}>
        <p>La traduction actuelle est gardée en sauvegarde : tu pourras y revenir.</p>
      </Confirmer>
    </Panel>
  );
}

const APPAREIL_ICONE = { clavier: 'command', souris: 'pin', manette: 'robot' };

function Touches({ jeuLance }) {
  const [profil, setProfil] = useState(null);
  const [liste, setListe] = useState([]);
  const [libelle, setLibelle] = useState('');
  const [occupe, setOccupe] = useState(false);
  const [retour, setRetour] = useState(null);
  const [aRestaurer, setARestaurer] = useState(null);
  const lire = useCallback(async () => {
    try { setProfil(await invoke('touches_lire')); } catch (e) { setProfil({ erreur: String(e), touches: [] }); }
    setListe(await invoke('touches_sauvegardes').catch(() => []));
  }, []);
  useEffect(() => { lire(); }, [lire]);

  const agir = async (commande, args, succes) => {
    setOccupe(true);
    setRetour(null);
    try {
      const r = await invoke(commande, args);
      if (r !== null && r !== undefined) setRetour({ severity: 'success', texte: succes ? succes(r) : String(r) });
    } catch (e) { setRetour({ severity: 'danger', texte: String(e) }); }
    finally { setOccupe(false); setARestaurer(null); lire(); }
  };

  if (!profil) return <Panel header="Touches du jeu"><p className="cmp-muted">Lecture du profil de touches…</p></Panel>;
  const parCategorie = profil.touches.reduce((acc, t) => { (acc[t.categorie] ??= []).push(t); return acc; }, {});

  return (
    <Panel header="Touches du jeu">
      <p className="cmp-muted">
        Le jeu n’enregistre que les touches que tu as <strong>modifiées</strong> : ce sont elles qui sont listées, sauvegardées et partagées.
      </p>
      {profil.erreur && <div className="cmp-gap"><Message severity="warn">{profil.erreur}</Message></div>}
      {!profil.present && !profil.erreur && <div className="cmp-gap"><Message severity="info">Aucune touche modifiée pour l’instant (profil par défaut du jeu).</Message></div>}
      {profil.touches.length > 0 && (
        <div className="cmp-touches">
          {Object.entries(parCategorie).map(([cat, ts]) => (
            <div key={cat}>
              <div className="cmp-touches__cat cmp-mono">{cat}</div>
              <ul>
                {ts.map((t, i) => (
                  <li key={`${t.action}-${i}`}>
                    <span className="cmp-mono">{t.action}</span>
                    <span className="cmp-touches__entree"><Icon name={APPAREIL_ICONE[t.appareil] ?? 'ship'} size={14} /> {t.entree}</span>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      )}
      {retour && <div className="cmp-gap"><Message severity={retour.severity}>{retour.texte}</Message></div>}

      <div className="cmp-gap cmp-ligne">
        <InputText aria-label="Nom de la sauvegarde (facultatif)" placeholder="Nom de la sauvegarde (facultatif)" value={libelle} maxLength={40}
          onChange={(e) => setLibelle(e.target.value)} />
        <Button label="Sauvegarder" icon="download" disabled={!profil.present} loading={occupe}
          onClick={() => agir('touches_sauvegarder', { libelle: libelle || null }, () => { setLibelle(''); return 'Touches sauvegardées.'; })} />
      </div>
      <div className="cmp-actions">
        <Button label="Exporter" variant="outlined" icon="upload" disabled={!profil.present} onClick={() => agir('touches_exporter', { fichier: null }, (c) => `Exporté : ${c}`)} />
        <Button label="Importer un profil" variant="outlined" icon="folder" onClick={() => agir('touches_importer', {})} />
      </div>

      {liste.length > 0 && (
        <>
          <div className="cmp-touches__cat cmp-gap">Sauvegardes</div>
          <ul className="cmp-events">
            {liste.map((s) => (
              <li key={s.fichier}>
                <Icon name="file" size={16} />
                <span className="cmp-events__txt">{s.libelle ?? 'Sauvegarde'} · {s.touches} touche(s)<br /><span className="cmp-muted cmp-small">{dateSauvegarde(s.date)}</span></span>
                <span className="cmp-ligne">
                  <Button size="sm" variant="text" label="Exporter" onClick={() => agir('touches_exporter', { fichier: s.fichier }, (c) => `Exporté : ${c}`)} />
                  <Button size="sm" variant="outlined" label="Restaurer" disabled={jeuLance} onClick={() => setARestaurer(s)} />
                </span>
              </li>
            ))}
          </ul>
          {jeuLance && <p className="cmp-muted">Ferme Star Citizen pour restaurer des touches (le jeu réécrit ce fichier en quittant).</p>}
        </>
      )}

      <Confirmer ouvert={!!aRestaurer} titre="Restaurer ces touches ?" libelle="Restaurer" occupe={occupe}
        onOui={() => agir('touches_restaurer', { fichier: aRestaurer.fichier })} onNon={() => setARestaurer(null)}>
        <p>« {aRestaurer?.libelle ?? 'Sauvegarde'} » remplacera tes touches actuelles, qui seront d’abord sauvegardées.</p>
        <p>Les touches s’appliqueront au prochain lancement du jeu.</p>
      </Confirmer>
    </Panel>
  );
}

const APPAREILS_LIBELLES = { keyboard: 'clavier', mouse: 'souris', gamepad: 'manette', joystick: 'joystick' };

/** Profils de touches partagés entre membres (décision du CES : l'auteur choisit « tout le CES » ou des membres). */
function ProfilsPartages({ compte, allerCompte }) {
  const [liste, setListe] = useState(null);
  const [erreur, setErreur] = useState(null);
  const [retour, setRetour] = useState(null);
  const [ouvert, setOuvert] = useState(false);
  const [membres, setMembres] = useState([]);
  const [filtre, setFiltre] = useState('');
  const [form, setForm] = useState({ nom: '', description: '', visibilite: 'ces', dest: [] });
  const [occupe, setOccupe] = useState(false);
  const relie = compte?.relie;

  const lire = useCallback(async () => {
    try { setListe(await invoke('touches_partages', { miens: false })); setErreur(null); }
    catch (e) { setErreur(String(e)); }
  }, []);
  useEffect(() => { if (relie) lire(); }, [relie, lire]);

  const ouvrir = async () => {
    setOuvert(true);
    setRetour(null);
    if (!membres.length) setMembres(await invoke('touches_membres').catch(() => []));
  };
  const basculerMembre = (id) => setForm((f) => ({ ...f, dest: f.dest.includes(id) ? f.dest.filter((x) => x !== id) : [...f.dest, id] }));
  const publier = async () => {
    setOccupe(true);
    try {
      const msg = await invoke('touches_publier', {
        nom: form.nom, description: form.description || null, visibilite: form.visibilite,
        destinataires: form.visibilite === 'members' ? form.dest : [], sauvegarde: null,
      });
      setRetour({ severity: 'success', texte: msg });
      setOuvert(false);
      setForm({ nom: '', description: '', visibilite: 'ces', dest: [] });
      lire();
    } catch (e) { setRetour({ severity: 'danger', texte: String(e), dansFenetre: true }); }
    finally { setOccupe(false); }
  };
  const agir = async (commande, id) => {
    setOccupe(true);
    try { setRetour({ severity: 'success', texte: await invoke(commande, { id }) }); }
    catch (e) { setRetour({ severity: 'danger', texte: String(e) }); }
    finally { setOccupe(false); lire(); }
  };

  if (!relie) {
    return (
      <Panel header="Profils de touches partagés">
        <p className="cmp-muted">Relie ton compte CES pour partager tes touches et récupérer celles des autres membres.</p>
        <div className="cmp-actions"><Button label="Relier mon compte" icon="link" onClick={allerCompte} /></div>
      </Panel>
    );
  }
  const membresFiltres = membres.filter((m) => m.username.toLowerCase().includes(filtre.toLowerCase()));

  return (
    <Panel header="Profils de touches partagés">
      <p className="cmp-muted">Partage tes touches modifiées avec le CES, ou récupère celles d’un autre membre (elles arrivent dans tes sauvegardes).</p>
      {retour && !retour.dansFenetre && <div className="cmp-gap"><Message severity={retour.severity}>{retour.texte}</Message></div>}
      {erreur && <div className="cmp-gap"><Message severity="warn">{erreur}</Message></div>}
      <div className="cmp-actions"><Button label="Partager mes touches" icon="users" onClick={ouvrir} /></div>

      {liste && liste.length === 0 && <p className="cmp-muted cmp-gap">Aucun profil partagé pour l’instant.</p>}
      {liste && liste.length > 0 && (
        <ul className="cmp-events cmp-gap">
          {liste.map((b) => (
            <li key={b.id}>
              <Icon name={b.visibility === 'members' ? 'lock' : 'users'} size={16} />
              <span className="cmp-events__txt">
                <strong>{b.name}</strong> · {b.mine ? 'toi' : b.author?.username}
                <br />
                <span className="cmp-muted cmp-small">
                  {(b.devices ?? []).map((d) => APPAREILS_LIBELLES[d] ?? d).join(', ') || 'touches'}
                  {b.gameVersion ? ` · jeu ${b.gameVersion}` : ''} · {heure(b.updatedAt ?? b.createdAt)}
                  {b.mine && b.visibility === 'members' ? ` · ${b.sharedWith?.length ?? 0} membre(s)` : ''}
                </span>
                {b.description && <><br /><span className="cmp-small">{b.description}</span></>}
              </span>
              <span className="cmp-ligne">
                {!b.mine && <Button size="sm" variant="outlined" label="Récupérer" loading={occupe} onClick={() => agir('touches_recuperer', b.id)} />}
                {b.mine && <Button size="sm" variant="text" severity="danger" label="Retirer" loading={occupe} onClick={() => agir('touches_retirer_partage', b.id)} />}
              </span>
            </li>
          ))}
        </ul>
      )}

      <Dialog open={ouvert} onClose={() => setOuvert(false)} title="Partager mes touches"
        footer={<><Button label="Annuler" variant="text" onClick={() => setOuvert(false)} /><Button label="Partager" loading={occupe} onClick={publier} /></>}>
        <div className="cmp-stack">
          {retour?.dansFenetre && <Message severity={retour.severity}>{retour.texte}</Message>}
          <Field label="Nom du profil" htmlFor="pt-nom">
            <InputText id="pt-nom" value={form.nom} maxLength={80} placeholder="ex. HOTAS Virpil — combat" onChange={(e) => setForm({ ...form, nom: e.target.value })} />
          </Field>
          <Field label="Description (facultatif)" htmlFor="pt-desc">
            <InputText id="pt-desc" value={form.description} maxLength={500} onChange={(e) => setForm({ ...form, description: e.target.value })} />
          </Field>
          <div>
            <div className="ds-label">Visible par</div>
            <SegmentedControl ariaLabel="Visible par" value={form.visibilite} onChange={(v) => setForm({ ...form, visibilite: v })}
              options={[{ value: 'ces', label: 'Tout le CES' }, { value: 'members', label: 'Membres choisis' }]} />
          </div>
          {form.visibilite === 'members' && (
            <div>
              <InputText aria-label="Chercher un membre" placeholder="Chercher un membre…" value={filtre} onChange={(e) => setFiltre(e.target.value)} />
              <div className="cmp-membres">
                {membresFiltres.map((m) => (
                  <Checkbox key={m.id} label={m.username} checked={form.dest.includes(m.id)} onChange={() => basculerMembre(m.id)} />
                ))}
                {!membresFiltres.length && <p className="cmp-muted">Aucun membre trouvé.</p>}
              </div>
              <p className="cmp-muted cmp-small">{form.dest.length} membre(s) choisi(s)</p>
            </div>
          )}
          <p className="cmp-muted cmp-small">Seules tes touches modifiées partent (le fichier du jeu, sans donnée personnelle).</p>
        </div>
      </Dialog>
    </Panel>
  );
}

/** À propos et licences (demande du CES) : mentions légales, sources de données, licences des composants. */
function APropos({ canal }) {
  const [liste, setListe] = useState(null); // 'rust' | 'js' | null
  const parLicence = (tab) => Object.entries(tab.reduce((a, x) => { a[x.licence] = (a[x.licence] || 0) + 1; return a; }, {}))
    .sort((a, b) => b[1] - a[1]);
  const portail = (chemin) => invoke('ouvrir_page', { url: `${__CES_PORTAIL__}${chemin}` }).catch(() => {});
  return (
    <div className="cmp-stack">
      <Panel header="CES Compagnon">
        <LogoCompagnon className="cmp-apropos__logo" />
        <dl className="cmp-dl">
          <dt>Version</dt><dd>{__APP_VERSION__}{canal ? ` · canal ${canal.actif}` : ''}</dd>
          <dt>Éditeur</dt><dd className="cmp-editeur"><img src={logoCesTexte} alt="Logo du CES" width="38" height="38" />CES — Corps Expéditionnaire Spatial (organisation de joueurs)</dd>
          <dt>Portail</dt><dd>{__CES_PORTAIL__.replace('https://', '')}</dd>
        </dl>
      </Panel>

      <Panel header="Star Citizen">
        <p className="cmp-muted cmp-small">
          Outil de fans <strong>non affilié à Cloud Imperium Games</strong>. Star Citizen®, Squadron 42®, Roberts Space Industries® et
          Cloud Imperium® sont des marques de Cloud Imperium Rights LLC et Cloud Imperium Rights Ltd. <em>Made by the Community.</em>
        </p>
        <p className="cmp-muted cmp-small cmp-gap">
          L’application ne touche pas au jeu : elle lit seulement les fichiers qu’il écrit (Game.log, réglages, touches) et, sur demande,
          une capture de l’écran. Ni lecture de la mémoire, ni injection, ni interception réseau.
        </p>
      </Panel>

      <Panel header="Sources de données">
        <ul className="cmp-sources">
          <li><strong>Traduction française</strong> — SPEED0U / SCEFRA (github.com/SPEED0U/Scefra), licence CC BY-NC-ND 4.0. Fichier téléchargé chez SCEFRA et adapté sur ce PC au format que lit le jeu (marque UTF-8, en-tête en commentaires « ; ») ; message d’accueil et page du journal sur Multitool remplacés par ceux du CES, avec le crédit SCEFRA ; aucune copie modifiée n’est publiée.</li>
          <li><strong>Prix, terminaux, raffineries</strong> — UEX Corp (uexcorp.space), estimations de la communauté, sans garantie.</li>
          <li><strong>Contrats de Wikelo</strong> — Star Citizen Wiki (api.star-citizen.wiki), usage non commercial ; images des objets : starcitizen.tools. Textes traduits en français pour CES Compagnon.</li>
          <li><strong>Lieux, vaisseaux, événements de jeu</strong> — fichier Game.log de ce PC, analysé localement.</li>
          <li><strong>Compte, partage, présence</strong> — portail CES ; seuls tes propres événements, selon tes réglages de partage.</li>
        </ul>
        <img src={uexBadge} alt="Powered by UEX" className="cmp-uex" width="180" />
      </Panel>

      <Panel header="Portail CES">
        <p className="cmp-muted cmp-small">Pages publiques du portail, ouvertes dans ton navigateur.</p>
        <div className="cmp-actions">
          <Button size="sm" variant="outlined" label="Mentions légales" onClick={() => portail('/legal')} />
          <Button size="sm" variant="outlined" label="Conditions d’utilisation" onClick={() => portail('/cgu')} />
          <Button size="sm" variant="outlined" label="Confidentialité" onClick={() => portail('/confidentialite')} />
          <Button size="sm" variant="outlined" icon="chart" label="Statut des services" onClick={() => invoke('statut_ouvrir').catch(() => {})} />
        </div>
      </Panel>

      <Panel header="Composants open source">
        <p className="cmp-muted cmp-small">
          {LICENCES.rust.length} composants Rust et {LICENCES.js.length} composants JavaScript (liste du {LICENCES.genere}), plus
          Aurore Design (design system du CES), les polices DM Sans, Ubuntu Mono et Orbitron (SIL Open Font License) et les icônes Remix Icon.
        </p>
        <ul className="cmp-sources cmp-small">
          {parLicence([...LICENCES.rust, ...LICENCES.js]).slice(0, 8).map(([l, n]) => <li key={l}><span className="cmp-mono">{l}</span> — {n}</li>)}
        </ul>
        <div className="cmp-actions">
          <Button size="sm" variant="text" label={liste === 'rust' ? 'Masquer' : 'Voir les composants Rust'} onClick={() => setListe(liste === 'rust' ? null : 'rust')} />
          <Button size="sm" variant="text" label={liste === 'js' ? 'Masquer' : 'Voir les composants JavaScript'} onClick={() => setListe(liste === 'js' ? null : 'js')} />
        </div>
        {liste && (
          <ul className="cmp-licences cmp-mono">
            {LICENCES[liste].map((c) => <li key={`${c.nom}@${c.version}`}><span>{c.nom} {c.version}</span><span>{c.licence}</span></li>)}
          </ul>
        )}
      </Panel>
    </div>
  );
}

const ETATS_SERVICE = {
  up: { texte: 'Opérationnel', severite: 'success' },
  degraded: { texte: 'Perturbé', severite: 'warn' },
  down: { texte: 'En panne', severite: 'danger' },
  maintenance: { texte: 'Maintenance', severite: 'info' },
};

/** Statut des services CES (CES Uptime, status.ces-sc.fr) : relu toutes les 2 minutes, silencieux hors ligne. */
function StatutCes() {
  const [s, setS] = useState(null);
  useEffect(() => {
    let actif = true;
    const lire = () => invoke('statut_ces').then((v) => { if (actif) setS(v); }).catch(() => { if (actif) setS((x) => x || { erreur: true }); });
    lire();
    const t = setInterval(lire, 120000);
    return () => { actif = false; clearInterval(t); };
  }, []);
  if (!s) return null;
  const g = ETATS_SERVICE[s.general] || { texte: 'Inconnu', severite: 'secondary' };
  return (
    <Panel header="Statut des services CES">
      {s.erreur ? <p className="cmp-muted">Page de statut injoignable pour le moment.</p> : (
        <>
          <div className="cmp-ligne"><Tag severity={g.severite} value={g.texte} /><span className="cmp-muted cmp-small">Vérifié {s.verifie ? new Date(s.verifie).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' }) : ''}</span></div>
          {s.incidents?.length > 0 && s.incidents.map((i) => <Message key={i.titre} severity={i.type === 'maintenance' ? 'info' : 'warn'}>{i.titre}</Message>)}
          <ul className="cmp-statut">
            {(s.services || []).map((sv) => {
              const e = ETATS_SERVICE[sv.etat] || { texte: sv.etat, severite: 'secondary' };
              return (
                <li key={sv.nom}>
                  <span>{sv.nom}</span>
                  <span className="cmp-muted cmp-small cmp-mono">{typeof sv.mois === 'number' ? `${sv.mois.toFixed(sv.mois === 100 ? 0 : 2)} % sur 30 j` : ''}</span>
                  <Tag severity={e.severite} value={e.texte} />
                </li>
              );
            })}
          </ul>
        </>
      )}
      <div className="cmp-actions"><Button size="sm" variant="text" icon="chart" label="Ouvrir la page de statut" onClick={() => invoke('statut_ouvrir').catch(() => {})} /></div>
    </Panel>
  );
}

/** Réglage CACHÉ des testeurs (5 clics sur le numéro de version) : canal de mises à jour. */
function CanalTesteur({ canal, onChange }) {
  const [msg, setMsg] = useState(null);
  const choisir = async (c) => {
    try { onChange(await invoke('canal_choisir', { canal: c })); setMsg(null); }
    catch (e) { setMsg(String(e)); }
  };
  return (
    <Panel header="Canal de mises à jour (testeurs)">
      <p className="cmp-muted">
        Les membres utilisent la version <strong>stable</strong>. La version <strong>canary</strong> est la version de test :
        plus récente, moins éprouvée. On ne redescend jamais de version : en changeant de canal, tu attends la prochaine version plus récente.
      </p>
      <div className="cmp-gap">
        <SegmentedControl ariaLabel="Canal" value={canal.actif} onChange={choisir}
          options={[{ value: 'stable', label: 'Stable' }, { value: 'canary', label: 'Canary (test)' }]} />
      </div>
      <p className="cmp-muted cmp-small cmp-gap">Canal de cette installation : {canal.build}. <button type="button" className="cmp-lien" onClick={() => choisir(null)}>Revenir au canal d’origine</button></p>
      {msg && <Message severity="danger">{msg}</Message>}
    </Panel>
  );
}

const NOTIFS = [
  { cle: 'lancements', titre: 'Lancements de Star Citizen', desc: '« X a lancé Star Citizen », pour les membres qui ont choisi de l’annoncer sur le portail.' },
  { cle: 'raccourcisSos', titre: 'SOS au clavier (même en jeu)', desc: 'Ctrl + Maj + 1 combat · 2 panne · 3 carburant · 4 médical · 5 transport · 6 autre. Appuie DEUX fois sur la même combinaison en moins de 3 s : le premier appui affiche une carte, le second envoie le SOS.' },
  { cle: 'carte', titre: 'Carte CES sur le bureau', desc: 'Lancements et SOS : une carte aux couleurs du CES en bas à droite de l’écran, par-dessus les autres fenêtres, au lieu de la notification Windows.' },
  { cle: 'defcon', titre: 'Alertes DEFCON', desc: 'Une notification à chaque changement de niveau.' },
  { cle: 'discord', titre: 'Discord Rich Presence', desc: 'Affiche « Dans le Verse » sur ton profil Discord pendant que tu joues (Discord doit être ouvert sur ce PC).' },
  { cle: 'portail', titre: 'Notifications du portail', desc: 'Inscriptions, mentions, invitations, résultats d’examen… comme dans la cloche. Désactive pour les mettre en sourdine.' },
  { cle: 'bandeau', titre: 'Alerte dans la fenêtre', desc: 'De DEFCON 3 à 1 : bandeau sous la barre de titre et liseré animé autour de la fenêtre de l’application.' },
];

/** Notifications Windows (toasts natifs) et bandeau DEFCON : réglages locaux, désactivables. */
function NotificationsWindows() {
  const [r, setR] = useState(null);
  const [discord, setDiscord] = useState(false);
  useEffect(() => { invoke('notif_reglages').then(setR).catch(() => {}); invoke('discord_disponible').then(setDiscord).catch(() => {}); }, []);
  if (!r) return null;
  const basculer = (cle, v) => {
    const neuf = { ...r, [cle]: v };
    setR(neuf);
    invoke('notif_reglages_ecrire', { reglages: neuf }).catch(() => setR(r));
  };
  return (
    <div data-visite="notifs" className="cmp-visite-cible"><Panel header="Notifications et Discord">
      <div className="cmp-reglages">
        {NOTIFS.filter((n) => n.cle !== 'discord' || discord).map((n) => (
          <SettingRow key={n.cle} title={n.titre} desc={n.desc} htmlFor={`nt-${n.cle}`}>
            <ToggleSwitch id={`nt-${n.cle}`} checked={r[n.cle] === true} onChange={(v) => basculer(n.cle, v)} />
          </SettingRow>
        ))}
      </div>
      <div className="cmp-actions">
        <Button size="sm" variant="outlined" label="Tester une notification" icon="bell" onClick={() => invoke('notif_tester').catch(() => {})} />
      </div>
    </Panel></div>
  );
}

/** État DEFCON lu dans le cœur (direct du portail) : bandeau et liseré de la fenêtre, de 3 à 1. */
function useDefcon(relie) {
  const [d, setD] = useState(null);
  useEffect(() => {
    if (!relie) { setD(null); return undefined; }
    const lire = () => invoke('notif_defcon').then(setD).catch(() => {});
    lire();
    const t = setInterval(lire, 4000);
    return () => clearInterval(t);
  }, [relie]);
  return [d, setD];
}

/** Navigation (haut, bas ou gauche, au choix de chacun) et démarrage avec Windows. */
function ReglagesInterface({ nav, setNav }) {
  const [demarrage, setDemarrage] = useState(null);
  const [erreur, setErreur] = useState(null);
  useEffect(() => { invoke('demarrage_etat').then(setDemarrage).catch(() => setDemarrage(false)); }, []);
  const basculer = async (v) => {
    setErreur(null);
    try { setDemarrage(await invoke('demarrage_regler', { actif: v })); } catch (e) { setErreur(String(e)); }
  };
  return (
    <Panel header="Interface">
      <SettingRow title="Position de la navigation" desc="Où placer la barre de navigation de l’application.">
        <SegmentedControl ariaLabel="Position de la navigation" value={nav} onChange={setNav}
          options={[{ value: 'gauche', label: 'À gauche' }, { value: 'haut', label: 'En haut' }, { value: 'bas', label: 'En bas' }]} />
      </SettingRow>
      <SettingRow title="Lancer CES Compagnon avec Windows" desc="L’application démarre avec le PC, rangée dans la zone de notification." htmlFor="rg-demarrage">
        <ToggleSwitch id="rg-demarrage" checked={!!demarrage} disabled={demarrage === null} onChange={basculer} />
      </SettingRow>
      {erreur && <Message severity="danger">{erreur}</Message>}
      <SettingRow title="Mises à jour" desc={`Version installée : ${__APP_VERSION__}. L’application vérifie aussi toute seule au démarrage puis toutes les 6 h.`}>
        <VerifierMaj />
      </SettingRow>
    </Panel>
  );
}

/** Vérification manuelle des mises à jour (demande du CES, 24/11). */
function VerifierMaj() {
  const [etat, setEtat] = useState(null); // null | 'encours' | 'ajour' | { erreur }
  const verifier = async () => {
    setEtat('encours');
    try {
      const u = await invoke('maj_verifier');
      if (u) { setEtat(null); window.dispatchEvent(new CustomEvent('cmp-maj-trouvee', { detail: u })); }
      else setEtat('ajour');
    } catch (e) { setEtat({ erreur: String(e) }); }
  };
  return (
    <span className="cmp-ligne">
      <Button size="sm" variant="outlined" icon="download" label="Vérifier les mises à jour" loading={etat === 'encours'} onClick={verifier} />
      {etat === 'ajour' && <span className="cmp-small" style={{ color: 'var(--ds-success)' }}>À jour ✓</span>}
      {etat?.erreur && <span className="cmp-small" style={{ color: 'var(--ds-danger)' }}>{etat.erreur}</span>}
    </span>
  );
}

function Outils({ etat, compte, allerCompte, testeur, canal, setCanal, fond, setFond, fondPerso, nav, setNav }) {
  const jeuLance = !!etat?.jeuLance;
  return (
    <div className="cmp-stack">
      <ReglagesInterface nav={nav} setNav={setNav} />
      <Panel header="Fond de l’application"><ChoixFond fond={fond} setFond={setFond} perso={fondPerso.perso} setPerso={fondPerso.setPerso} /></Panel>
      <NotificationsWindows />
      <ProfilsPartages compte={compte} allerCompte={allerCompte} />
      {testeur && canal && <CanalTesteur canal={canal} onChange={setCanal} />}
      <Message severity="info">Version, licences, sources et mentions : écran « À propos ».</Message>
    </div>
  );
}

const auec = (n) => `${Math.round(n).toLocaleString('fr-FR')} aUEC`;
const ageTexte = (h) => (h < 1 ? 'moins d’1 h' : h < 48 ? `${Math.round(h)} h` : `${Math.round(h / 24)} j`);

/** Routes de commerce : données UEX (créditées), soute du vaisseau choisi, calcul sur le PC. */
/** Jeton d'application UEX propre au membre (facultatif) : quota personnel, rangé dans le gestionnaire d'identification. */
function JetonUex() {
  const [present, setPresent] = useState(null);
  const [ouvert, setOuvert] = useState(false);
  const [valeur, setValeur] = useState('');
  const [msg, setMsg] = useState(null);
  const [occupe, setOccupe] = useState(false);
  useEffect(() => { invoke('uex_jeton_etat').then((e) => setPresent(e.present)).catch(() => {}); }, []);
  const enregistrer = async () => {
    setOccupe(true); setMsg(null);
    try { await invoke('uex_jeton_enregistrer', { jeton: valeur }); setPresent(true); setValeur(''); setOuvert(false); setMsg({ s: 'success', t: 'Jeton vérifié auprès d’UEX et enregistré dans Windows.' }); }
    catch (e) { setMsg({ s: 'danger', t: String(e) }); }
    finally { setOccupe(false); }
  };
  const retirer = () => invoke('uex_jeton_effacer').then(() => { setPresent(false); setMsg({ s: 'info', t: 'Jeton retiré : l’application utilise de nouveau l’accès public d’UEX.' }); }).catch(() => {});
  if (present === null) return null;
  return (
    <Panel header="Clé d’API UEX (facultative)">
      <div className="cmp-ligne">
        <Tag severity={present ? 'success' : 'secondary'} value={present ? 'Jeton personnel enregistré' : 'Accès public partagé'} />
        <span className="cmp-muted cmp-small">{present ? 'Quota personnel UEX (environ 170 000 requêtes par jour).' : 'Ça marche sans jeton, avec la limite commune à tous les utilisateurs.'}</span>
      </div>
      {msg && <Message severity={msg.s}>{msg.t}</Message>}
      {!ouvert ? (
        <div className="cmp-actions">
          <Button size="sm" variant="outlined" icon="lock" label={present ? 'Remplacer mon jeton' : 'Ajouter mon jeton UEX'} onClick={() => setOuvert(true)} />
          {present && <Button size="sm" variant="text" severity="danger" label="Retirer" onClick={retirer} />}
        </div>
      ) : (
        <>
          <ol className="cmp-etapes cmp-small">
            <li>Ouvre la page des applications d’UEX et connecte-toi (compte UEX gratuit). <Button size="sm" variant="text" icon="link" label="Ouvrir uexcorp.space/api/apps" onClick={() => invoke('uex_ouvrir_page').catch(() => {})} /></li>
            <li>Clique sur « Commencez dès maintenant ! » (ou « Créer une application »).</li>
            <li>Lis et accepte les conditions d’utilisation de l’API UEX, donne un nom à ton application (par exemple « CES Compagnon »), puis « Créer ».</li>
            <li>Sur la page de ton application, déplie « Jeton Bearer » et copie le jeton (40 caractères).</li>
            <li>Colle-le ci-dessous. Il est vérifié auprès d’UEX, puis rangé dans le gestionnaire d’identification de Windows ; il ne quitte ton PC que vers UEX.</li>
          </ol>
          <div className="cmp-ligne">
            <InputText type="password" aria-label="Jeton UEX" placeholder="Jeton Bearer UEX" value={valeur} autoComplete="off" spellCheck={false} onChange={(e) => setValeur(e.target.value.trim())} />
            <Button size="sm" label="Enregistrer" loading={occupe} disabled={valeur.length < 40} onClick={enregistrer} />
            <Button size="sm" variant="text" label="Annuler" onClick={() => { setOuvert(false); setValeur(''); }} />
          </div>
          <p className="cmp-muted cmp-small">Si UEX change ou révoque ton jeton, crée-en un nouveau et colle-le ici : pas besoin de mise à jour de l’application.</p>
        </>
      )}
    </Panel>
  );
}

function Commerce() {
  const [d, setD] = useState(null);
  const [erreur, setErreur] = useState(null);
  const [vaisseau, setVaisseau] = useState('');
  const [scu, setScu] = useState('');
  const [budget, setBudget] = useState('');
  const [systeme, setSysteme] = useState('');
  const [depart, setDepart] = useState('');
  const [illegales, setIllegales] = useState(false);
  const [routes, setRoutes] = useState(null);
  const [occupe, setOccupe] = useState(false);

  const charger = useCallback(async (forcer = false) => {
    setOccupe(true);
    try { setD(await invoke('commerce_donnees', { forcer })); setErreur(null); }
    catch (e) { setErreur(String(e)); }
    finally { setOccupe(false); }
  }, []);
  useEffect(() => { charger(); }, [charger]);

  const choisirVaisseau = (i) => {
    setVaisseau(i);
    const v = d?.vaisseaux[Number(i)];
    if (v) setScu(String(v.scu));
  };
  const calculer = async () => {
    setOccupe(true);
    try {
      setRoutes(await invoke('commerce_routes', { parametres: {
        scu: Number(scu) || 0,
        budget: budget ? Number(budget) : null,
        systeme: systeme || null,
        depart: depart ? Number(depart) : null,
        illegales,
      } }));
      setErreur(null);
    } catch (e) { setErreur(String(e)); }
    finally { setOccupe(false); }
  };

  if (!d && !erreur) return <p className="cmp-muted">Chargement des prix (UEX)…</p>;
  const hangar = (d?.vaisseaux ?? []).map((v, i) => ({ ...v, i })).filter((v) => v.source === 'hangar');
  const autres = (d?.vaisseaux ?? []).map((v, i) => ({ ...v, i })).filter((v) => v.source === 'uex');
  const terminaux = (d?.terminaux ?? []).filter((t) => !systeme || t.systeme === systeme);

  return (
    <div className="cmp-stack">
      {erreur && <Message severity="danger">{erreur}</Message>}
      <Panel header="Routes de commerce">
        <div className="cmp-form">
          <Field label="Vaisseau" htmlFor="cm-v">
            <select id="cm-v" className="ds-select" value={vaisseau} onChange={(e) => choisirVaisseau(e.target.value)}>
              <option value="">Choisir…</option>
              {hangar.length > 0 && <optgroup label="Mon hangar">{hangar.map((v) => <option key={v.i} value={v.i}>{v.nom} — {v.scu} SCU</option>)}</optgroup>}
              <optgroup label="Tous les vaisseaux de transport">{autres.map((v) => <option key={v.i} value={v.i}>{v.nom} — {v.scu} SCU</option>)}</optgroup>
            </select>
          </Field>
          <div className="cmp-deux">
            <Field label="Soute (SCU)" htmlFor="cm-scu"><InputText id="cm-scu" inputMode="numeric" value={scu} onChange={(e) => setScu(e.target.value.replace(/[^\d.]/g, ''))} /></Field>
            <Field label="Budget (aUEC, facultatif)" htmlFor="cm-b"><InputText id="cm-b" inputMode="numeric" value={budget} onChange={(e) => setBudget(e.target.value.replace(/\D/g, ''))} /></Field>
          </div>
          <div className="cmp-deux">
            <Field label="Système" htmlFor="cm-s">
              <Select id="cm-s" value={systeme} onChange={(e) => { setSysteme(e.target.value); setDepart(''); }}
                options={[{ value: '', label: 'Tous' }, ...(d?.systemes ?? []).map((s) => ({ value: s, label: s }))]} />
            </Field>
            <Field label="Départ (facultatif)" htmlFor="cm-d">
              <Select id="cm-d" value={depart} onChange={(e) => setDepart(e.target.value)}
                options={[{ value: '', label: 'N’importe où' }, ...terminaux.map((t) => ({ value: String(t.id), label: `${t.nom}${t.lieu ? ` (${t.lieu})` : ''}` }))]} />
            </Field>
          </div>
          <Checkbox label="Inclure les marchandises illégales" checked={illegales} onChange={(e) => setIllegales(e.target.checked)} />
        </div>
        {d?.hangarErreur && <p className="cmp-muted cmp-small cmp-gap">{d.hangarErreur}</p>}
        <div className="cmp-actions">
          <Button label="Calculer les routes" variant="aurora" icon="chart" loading={occupe} disabled={!Number(scu)} onClick={calculer} />
          <Button label="Actualiser les prix" variant="text" icon="redo" onClick={() => charger(true)} />
        </div>
        {d && <p className="cmp-muted cmp-small cmp-gap">{d.nbPrix.toLocaleString('fr-FR')} prix · reçus à {heure(new Date(d.prixRecusA * 1000).toISOString())} · prix de plus de 7 jours ignorés</p>}
      </Panel>

      {routes && routes.length === 0 && <EmptyState icon="truck" title="Aucune route rentable" text="Essaie un autre système, un autre départ ou un budget plus élevé." />}
      {routes && routes.length > 0 && (
        <ul className="cmp-routes">
          {routes.map((r, i) => (
            <li key={i} className="cmp-route">
              <div className="cmp-route__tete">
                <strong>{r.marchandise}</strong>{r.illegale && <Tag severity="danger" value="illégale" />}
                <span className="cmp-route__gain">+{auec(r.benefice)}</span>
              </div>
              <div className="cmp-route__etapes">
                <div><span className="cmp-muted cmp-small">Acheter</span><br />{r.achat.terminal}{r.achat.lieu && <span className="cmp-muted"> · {r.achat.lieu}</span>}<br />
                  <span className="cmp-mono">{r.achat.prix.toLocaleString('fr-FR')}</span> <span className="cmp-muted cmp-small">/SCU · prix de {ageTexte(r.achat.ageHeures)}</span></div>
                <Icon name="right" size={18} />
                <div><span className="cmp-muted cmp-small">Vendre</span><br />{r.vente.terminal}{r.vente.lieu && <span className="cmp-muted"> · {r.vente.lieu}</span>}<br />
                  <span className="cmp-mono">{r.vente.prix.toLocaleString('fr-FR')}</span> <span className="cmp-muted cmp-small">/SCU · prix de {ageTexte(r.vente.ageHeures)}</span></div>
              </div>
              <div className="cmp-muted cmp-small">
                {r.scu} SCU · mise {auec(r.investissement)} · marge {r.margeUnitaire.toLocaleString('fr-FR')} /SCU
                {r.achat.systeme !== r.vente.systeme ? ` · ${r.achat.systeme} → ${r.vente.systeme}` : ''}
                {r.conteneurMax ? ` · conteneurs ≤ ${r.conteneurMax} SCU` : ''}
              </div>
            </li>
          ))}
        </ul>
      )}
      <p className="cmp-muted cmp-small">Données de prix : <strong>UEX Corp</strong> (uexcorp.space), estimations de la communauté, sans garantie. Vérifie les prix sur place.</p>
    </div>
  );
}

const dureeEntre = (a, b) => {
  const min = Math.max(0, Math.round((new Date(b) - new Date(a)) / 60000));
  return min < 60 ? `${min} min` : `${Math.floor(min / 60)} h ${String(min % 60).padStart(2, '0')}`;
};
const nf = (n, d = 0) => Number(n || 0).toLocaleString('fr-FR', { maximumFractionDigits: d });

/** Lecture À LA DEMANDE de l'écran de raffinerie : compte à rebours, une capture, OCR Windows, proposition à valider. */
function OcrRaffinerie({ minerais, onAjouter }) {
  const [etat, setEtat] = useState(null); // null | { compte } | 'lecture' | { detections, lignes } | { erreur }
  const [choix, setChoix] = useState([]);
  const lancer = async () => {
    const DELAI = 5;
    for (let i = DELAI; i > 0; i -= 1) {
      setEtat({ compte: i });
      // eslint-disable-next-line no-await-in-loop
      await new Promise((r) => setTimeout(r, 1000));
    }
    setEtat('lecture');
    try {
      const r = await invoke('ocr_raffinerie', { delai: 0, minerais: minerais.map((m) => m.nom) });
      setChoix(r.detections.map((d) => ({ ...d, garder: true })));
      setEtat(r);
    } catch (e) { setEtat({ erreur: String(e) }); }
  };
  const valider = () => {
    onAjouter(choix.filter((c) => c.garder && c.scu > 0).map(({ nom, scu }) => ({ nom, scu })));
    setEtat(null);
  };

  if (!etat) {
    return (
      <div className="cmp-gap">
        <Button label="Lire l’écran de la raffinerie" variant="text" icon="eye" onClick={lancer} />
        <span className="cmp-muted cmp-small"> Une seule capture, après 5 s pour revenir au jeu. Rien n’est enregistré ni envoyé.</span>
      </div>
    );
  }
  if (etat.compte) return <Message severity="info" title={`Capture dans ${etat.compte} s…`}>Reviens sur l’écran de la raffinerie dans le jeu (mode fenêtré sans bordure).</Message>;
  if (etat === 'lecture') return <p className="cmp-muted cmp-gap">Lecture de l’écran…</p>;
  if (etat.erreur) return <Message severity="danger" onClose={() => setEtat(null)}>{etat.erreur}</Message>;
  return (
    <div className="cmp-ocr">
      {choix.length === 0
        ? <Message severity="warn" onClose={() => setEtat(null)}>Aucun minerai reconnu ({etat.lignes.length} lignes lues). L’écran de la raffinerie était-il visible ?</Message>
        : (
          <>
            <div className="ds-label">Reconnus à l’écran — vérifie avant d’ajouter</div>
            {choix.map((c, i) => (
              <div key={c.nom} className="cmp-ligne">
                <Checkbox label={c.nom} checked={c.garder} onChange={(e) => setChoix(choix.map((x, j) => (j === i ? { ...x, garder: e.target.checked } : x)))} />
                <InputText aria-label={`SCU ${c.nom}`} inputMode="decimal" style={{ maxWidth: 90, marginLeft: 'auto' }} value={c.scu}
                  onChange={(e) => setChoix(choix.map((x, j) => (j === i ? { ...x, scu: Number(e.target.value.replace(',', '.')) || 0 } : x)))} />
              </div>
            ))}
            <div className="cmp-actions">
              <Button label="Ajouter à la session" icon="plus" onClick={valider} />
              <Button label="Annuler" variant="text" onClick={() => setEtat(null)} />
            </div>
          </>
        )}
    </div>
  );
}

/** Session de minage en cours : minerais, raffinerie, méthode, gains (saisie manuelle). */
function SessionMinage({ session, ref_, onFin, onMaj }) {
  const [s, setS] = useState(session);
  const [ligne, setLigne] = useState({ nom: '', scu: '' });
  const [occupe, setOccupe] = useState(false);
  const [msg, setMsg] = useState(null);
  const [, tic] = useState(0);
  useEffect(() => { const t = setInterval(() => tic((x) => x + 1), 30000); return () => clearInterval(t); }, []);
  useEffect(() => setS(session), [session]);

  const minerais = ref_?.minerais ?? [];
  const parNom = Object.fromEntries(minerais.map((m) => [m.nom, m]));
  const raff = (ref_?.raffineries ?? []).find((r) => r.nom === s.raffinerie);
  const estimation = s.minerais.reduce((t, m) => t + m.scu * (parNom[m.nom]?.prixVente || 0), 0);

  const ajouter = () => {
    const scu = Number(String(ligne.scu).replace(',', '.'));
    if (!ligne.nom || !(scu > 0)) return;
    setS({ ...s, minerais: [...s.minerais, { nom: ligne.nom, scu }] });
    setLigne({ nom: '', scu: '' });
  };
  const enregistrer = async (terminer) => {
    setOccupe(true);
    try {
      const m = await invoke('minage_enregistrer', { session: s, terminer });
      setMsg({ severity: 'success', texte: m });
      if (terminer) onFin(m); else onMaj();
    } catch (e) { setMsg({ severity: 'danger', texte: String(e) }); }
    finally { setOccupe(false); }
  };

  return (
    <Panel header={`Session en cours · ${dureeEntre(s.debut, new Date())}`}>
      <div className="cmp-form">
        <div className="cmp-deux">
          <Field label="Vaisseau" htmlFor="mn-v"><InputText id="mn-v" value={s.vaisseau ?? ''} maxLength={80} placeholder="ex. Prospector" onChange={(e) => setS({ ...s, vaisseau: e.target.value })} /></Field>
          <Field label="Lieu" htmlFor="mn-l"><InputText id="mn-l" value={s.lieu ?? ''} maxLength={120} placeholder="ex. Aaron Halo" onChange={(e) => setS({ ...s, lieu: e.target.value })} /></Field>
        </div>

        <div>
          <div className="ds-label">Minerais extraits</div>
          {s.minerais.length > 0 && (
            <ul className="cmp-touches cmp-mn-liste">
              {s.minerais.map((m, i) => (
                <li key={i}>
                  <span>{m.nom}</span>
                  <span className="cmp-ligne">
                    <span className="cmp-mono">{nf(m.scu, 2)} SCU</span>
                    <Button size="sm" variant="text" label="Retirer" onClick={() => setS({ ...s, minerais: s.minerais.filter((_, j) => j !== i) })} />
                  </span>
                </li>
              ))}
            </ul>
          )}
          <div className="cmp-ligne cmp-gap">
            <select className="ds-select" aria-label="Minerai" value={ligne.nom} onChange={(e) => setLigne({ ...ligne, nom: e.target.value })}>
              <option value="">Minerai…</option>
              {minerais.map((m) => <option key={m.nom} value={m.nom}>{m.nom}</option>)}
            </select>
            <InputText aria-label="Quantité en SCU" placeholder="SCU" inputMode="decimal" style={{ maxWidth: 80 }} value={ligne.scu}
              onChange={(e) => setLigne({ ...ligne, scu: e.target.value.replace(/[^\d.,]/g, '') })} onKeyDown={(e) => e.key === 'Enter' && ajouter()} />
            <Button label="Ajouter" icon="plus" variant="outlined" onClick={ajouter} />
          </div>
          <OcrRaffinerie minerais={minerais} onAjouter={(lignes) => setS({ ...s, minerais: [...s.minerais, ...lignes] })} />
        </div>

        <div className="cmp-deux">
          <Field label="Raffinerie" htmlFor="mn-r">
            <select id="mn-r" className="ds-select" value={s.raffinerie ?? ''} onChange={(e) => setS({ ...s, raffinerie: e.target.value || null })}>
              <option value="">—</option>
              {(ref_?.raffineries ?? []).map((r) => <option key={r.nom} value={r.nom}>{r.nom.replace(/^Refinement Center - /, '')}</option>)}
            </select>
          </Field>
          <Field label="Méthode" htmlFor="mn-m">
            <select id="mn-m" className="ds-select" value={s.methode ?? ''} onChange={(e) => setS({ ...s, methode: e.target.value || null })}>
              <option value="">—</option>
              {(ref_?.methodes ?? []).map((m) => <option key={m.nom} value={m.nom}>{m.nom} (rendement {m.rendement}/3, vitesse {m.vitesse}/3)</option>)}
            </select>
          </Field>
        </div>
        {raff && s.minerais.some((m) => raff.bonus[m.nom] !== undefined) && (
          <p className="cmp-muted cmp-small">
            Bonus de cette raffinerie : {s.minerais.filter((m) => raff.bonus[m.nom] !== undefined).map((m) => `${m.nom} ${raff.bonus[m.nom] > 0 ? '+' : ''}${raff.bonus[m.nom]} %`).join(' · ')}
          </p>
        )}
        <div className="cmp-deux">
          <Field label="Gains réels (aUEC, à la vente)" htmlFor="mn-g">
            <InputText id="mn-g" inputMode="numeric" value={s.gains ?? ''} onChange={(e) => setS({ ...s, gains: e.target.value ? Number(e.target.value.replace(/\D/g, '')) : null })} />
          </Field>
          <div>
            <div className="ds-label">Estimation (UEX)</div>
            <div className="cmp-mono cmp-mn-estim">{estimation ? `≈ ${nf(estimation)} aUEC` : '—'}</div>
          </div>
        </div>
        <Field label="Notes" htmlFor="mn-n"><InputText id="mn-n" value={s.notes ?? ''} maxLength={1000} onChange={(e) => setS({ ...s, notes: e.target.value })} /></Field>
      </div>
      {msg && <div className="cmp-gap"><Message severity={msg.severity}>{msg.texte}</Message></div>}
      <div className="cmp-actions">
        <Button label="Terminer la session" variant="aurora" icon="check" loading={occupe} onClick={() => enregistrer(true)} />
        <Button label="Enregistrer le brouillon" variant="outlined" loading={occupe} onClick={() => enregistrer(false)} />
      </div>
      <p className="cmp-muted cmp-small cmp-gap">Estimation : SCU × meilleur prix de vente récent du minerai raffiné (UEX), avant pertes de raffinage.</p>
    </Panel>
  );
}

function Minage({ compte }) {
  const [ref_, setRef] = useState(null);
  const [b, setB] = useState(null);
  const [erreur, setErreur] = useState(null);
  const [retour, setRetour] = useState(null);
  const relire = useCallback(() => invoke('minage_bilan').then(setB).catch((e) => setErreur(String(e))), []);
  useEffect(() => {
    relire();
    invoke('minage_referentiel', {}).then(setRef).catch((e) => setErreur(String(e)));
  }, [relire]);

  const commencer = async () => {
    setRetour(null);
    try { await invoke('minage_commencer', { vaisseau: null }); relire(); }
    catch (e) { setErreur(String(e)); }
  };
  const supprimer = async (id) => { await invoke('minage_supprimer', { id }).catch(() => {}); relire(); };
  const exporter = async () => {
    try { const c = await invoke('minage_exporter'); if (c) setRetour({ severity: 'success', texte: `Exporté : ${c}` }); }
    catch (e) { setRetour({ severity: 'danger', texte: String(e) }); }
  };

  if (!b) return <p className="cmp-muted">Chargement…</p>;
  const partage = compte?.relie && compte?.reglages?.mining === true;

  return (
    <div className="cmp-stack">
      {erreur && <Message severity="warn">{erreur}</Message>}
      {retour && <Message severity={retour.severity}>{retour.texte}</Message>}
      {b.enCours
        ? <SessionMinage session={b.enCours} ref_={ref_} onMaj={relire} onFin={(m) => { setRetour({ severity: 'success', texte: m }); relire(); }} />
        : (
          <Panel header="Minage">
            <p className="cmp-muted">Note tes sessions : minerais, raffinerie, gains. Le journal du jeu ne donne pas ces informations : la saisie est manuelle.</p>
            <div className="cmp-actions"><Button label="Commencer une session" variant="aurora" icon="plus" onClick={commencer} /></div>
            <p className="cmp-muted cmp-small cmp-gap">
              {partage ? 'Les sessions terminées sont partagées avec le portail (réglage « Sessions de minage »).' : 'Les sessions restent sur ce PC. Pour les partager avec le portail, active « Sessions de minage » dans l’onglet Partage.'}
            </p>
          </Panel>
        )}

      {b.nb > 0 && (
        <Panel header="Historique">
          <div className="cmp-mn-totaux">
            <div><strong>{b.nb}</strong><span className="cmp-muted cmp-small">sessions</span></div>
            <div><strong>{nf(b.totalScu, 1)}</strong><span className="cmp-muted cmp-small">SCU</span></div>
            <div><strong>{nf(b.totalGains)}</strong><span className="cmp-muted cmp-small">aUEC</span></div>
            <div><strong>{b.totalHeures > 0 ? nf(b.totalGains / b.totalHeures) : '—'}</strong><span className="cmp-muted cmp-small">aUEC / h</span></div>
          </div>
          <ul className="cmp-events cmp-gap">
            {b.sessions.map((s) => (
              <li key={s.id}>
                <Icon name={s.envoye ? 'users' : 'file'} size={16} />
                <span className="cmp-events__txt">
                  {heure(s.debut)} · {dureeEntre(s.debut, s.fin)} · {nf(s.minerais.reduce((t, m) => t + m.scu, 0), 1)} SCU{s.gains ? ` · ${nf(s.gains)} aUEC` : ''}
                  <br /><span className="cmp-muted cmp-small">{s.minerais.map((m) => m.nom).join(', ') || 'aucun minerai noté'}{s.raffinerie ? ` · ${s.raffinerie.replace(/^Refinement Center - /, '')}` : ''}{s.envoye ? ' · partagée' : ''}</span>
                </span>
                <Button size="sm" variant="text" label="Supprimer" onClick={() => supprimer(s.id)} />
              </li>
            ))}
          </ul>
          <div className="cmp-actions"><Button label="Exporter (CSV)" variant="outlined" icon="upload" onClick={exporter} /></div>
          <p className="cmp-muted cmp-small cmp-gap">Supprimer ici n’efface pas une session déjà partagée : fais-le sur le portail (Profil › Sécurité › CES Compagnon).</p>
        </Panel>
      )}
      <p className="cmp-muted cmp-small">Minerais, raffineries et prix : <strong>UEX Corp</strong> (uexcorp.space), estimations de la communauté.</p>
    </div>
  );
}

const CATEGORIES_WK = [
  { value: 'tous', label: 'Tous' },
  { value: 'favoris', label: '★ Suivis' },
  { value: 'vaisseaux', label: 'Vaisseaux' },
  { value: 'armures', label: 'Armures' },
  { value: 'armes', label: 'Armes' },
  { value: 'faveurs', label: 'Faveurs' },
  { value: 'autres', label: 'Autres' },
];

/** Un contrat de Wikelo, avec le suivi des objets réunis (local). */
// Noms d'objets du wiki (anglais) → français, pour les mots les plus courants (le suivi garde le nom anglais comme clé).
const GLOSSAIRE_WK = [
  [/^Wikelo Favor$/i, 'Faveur de Wikelo'],
  [/\s*\(Ore\)$/i, ' (minerai)'], [/\s*\(Pure\)$/i, ' (pur)'], [/\s*\(Raw\)$/i, ' (brut)'], [/\s*\(Gem\)$/i, ' (gemme)'],
  [/\bHelmet\b/, 'Casque'], [/\bLegs\b/, 'Jambes'], [/\bArms\b/, 'Bras'], [/\bCore\b/, 'Torse'], [/\bBackpack\b/, 'Sac à dos'],
  [/\bUndersuit\b/, 'Sous-combinaison'], [/\bScrip\b/, 'jetons'], [/\bWhite\b/, 'blanc'], [/\bBlack\b/, 'noir'], [/\bRed\b/, 'rouge'],
  [/\bTooth\b/, 'dent'], [/\bHorn\b/, 'corne'], [/\bFavor\b/, 'Faveur'], [/\bPolaris Bit\b/, 'Morceau de Polaris'],
];
const nomFr = (n) => GLOSSAIRE_WK.reduce((t, [re, fr]) => t.replace(re, fr), String(n || ''));
/** Texte du jeu : balises de mise en forme retirées, destination remplacée. */
const texteJeu = (t) => String(t || '').replace(/<EM\d>\[destination\|ListAll\]<\/EM\d>/g, 'l’Emporium de Wikelo').replace(/<\/?EM\d>/g, '');

/** Images Wikelo servies par le portail (GET /api/wikelo) : contrat → couverture et objets (nom → uuid, image). */
function indexerWikeloPortail(p) {
  const m = new Map();
  for (const c of p?.contracts || []) {
    const objets = new Map();
    for (const x of [...(c.rewards || []), ...(c.needs || [])]) objets.set(String(x.name || '').toLowerCase(), { uuid: x.uuid, image: x.image ? mediaPortail(x.image) : null, source: x.imageFrom || (x.image ? 'wiki' : null) });
    const r = (c.rewards || []).find((x) => x.image);
    m.set(c.uuid, { couverture: r ? mediaPortail(r.image) : null, objets });
  }
  return m;
}

function ContratWikelo({ c, suivi, onFavori, onReunir, onImage, pw, portailPret, onProposer }) {
  const fr = WIKELO_FR.contrats[c.uuid];
  const [detail, setDetail] = useState(null);
  const [ouvert, setOuvert] = useState(false);
  const charger = () => { if (!detail) invoke('wikelo_detail', { uuid: c.uuid }).then(setDetail).catch(() => setDetail({ erreur: true })); };
  // Image d'un objet : celle du portail d'abord (notre serveur, rapide, et remplacée par la modération si besoin), sinon le wiki.
  const objetPortail = (nom) => pw?.objets.get(String(nom || '').toLowerCase());
  const imageDe = (nom, repli) => objetPortail(nom)?.image || repli || null;
  const couverture = pw?.couverture || detail?.recompenses?.find((r) => r.image)?.image;
  const BoutonProposer = ({ nom }) => {
    const o = objetPortail(nom);
    return o?.uuid && onProposer ? <button type="button" className="cmp-lien cmp-small" onClick={() => onProposer({ uuid: o.uuid, nom: nomFr(nom) })}>{o.image ? 'Proposer une autre image' : 'Proposer une image'}</button> : null;
  };
  // Sans le portail, l'image de la récompense vient du wiki : chargée dès que la carte apparaît (fiches en cache sur le disque).
  const refCarte = useRef(null);
  useEffect(() => {
    const el = refCarte.current;
    if (!el || detail || !portailPret || pw?.couverture) return undefined;
    const obs = new IntersectionObserver((e) => { if (e.some((x) => x.isIntersecting)) { obs.disconnect(); charger(); } }, { rootMargin: '200px' });
    obs.observe(el);
    return () => obs.disconnect();
  }, [detail, portailPret, pw]);
  const reunis = suivi.reunis?.[c.uuid] ?? {};
  const fait = c.elements.filter((e) => (reunis[e.nom] ?? 0) >= e.quantite).length;
  const favori = suivi.favoris?.includes(c.uuid);
  return (
    <li ref={refCarte} className={`wkc${ouvert ? ' is-ouvert' : ''}${fait === c.elements.length && c.elements.length ? ' is-pret' : ''}`}>
      <button type="button" className="wkc__visuel" onClick={() => { setOuvert(!ouvert); charger(); }} aria-expanded={ouvert}
        style={couverture ? { backgroundImage: `url("${couverture}")` } : undefined}>
        {!couverture && <Icon name="star" size={28} />}
        <span className="wkc__cat">{(CATEGORIES_WK.find((x) => x.value === c.categorie) || {}).label || c.categorie}</span>
      </button>
      <div className="wkc__corps">
        <div className="wkc__tete">
          <strong title={fr ? `Titre d’origine : ${c.titre}` : undefined}>{fr?.titre || c.titre}</strong>
          <button type="button" className="cmp-etoile" aria-pressed={favori} aria-label={favori ? 'Ne plus suivre' : 'Suivre ce contrat'}
            onClick={() => onFavori(c.uuid, !favori)}>{favori ? '★' : '☆'}</button>
        </div>
        <div className="wkc__infos cmp-small">
          {!c.disponible && <Tag value="indisponible" />}
          <span className="cmp-muted">{c.elements.length} objet{c.elements.length > 1 ? 's' : ''}{c.reputation ? ` · +${c.reputation} réputation` : ''}</span>
          <span className="wkc__compte">{fait}/{c.elements.length}</span>
        </div>
        <ProgressBar value={c.elements.length ? Math.round((fait / c.elements.length) * 100) : 0} />
        <button type="button" className="cmp-lien" onClick={() => { setOuvert(!ouvert); charger(); }}>{ouvert ? 'Masquer' : 'Voir le contrat'}</button>
      {ouvert && (
        <>
          <p className="cmp-small cmp-wk-texte">{fr ? fr.description : texteJeu(c.description)}</p>
          {fr && <p className="cmp-muted cmp-small">Titre d’origine : {c.titre}</p>}
          {detail?.recompenses?.length > 0 && (
            <div className="cmp-wk-recompenses">
              {detail.recompenses.map((r) => { const img = imageDe(r.nom, r.image); return (
                <figure key={r.nom} className="cmp-wk-objet">
                  {img
                    ? <button type="button" className="cmp-wk-zoom" onClick={() => onImage({ src: img, titre: nomFr(r.nom), sous: r.type })} aria-label={`Voir ${nomFr(r.nom)} en grand`}><img src={img} alt="" loading="lazy" /></button>
                    : <span className="cmp-wk-objet__vide"><Icon name="star" size={22} /></span>}
                  <figcaption>
                    <strong>{nomFr(r.nom)}{r.quantite > 1 ? ` × ${r.quantite}` : ''}</strong>
                    {r.type && <span className="cmp-muted cmp-small">{r.type}</span>}
                    <BoutonProposer nom={r.nom} />
                    {r.description && <details><summary className="cmp-small">Description</summary><p className="cmp-small">{r.description}</p></details>}
                  </figcaption>
                </figure>
              ); })}
            </div>
          )}
          {!detail && <p className="cmp-muted cmp-small">Chargement des objets…</p>}
          {detail?.erreur && <p className="cmp-muted cmp-small">Images indisponibles pour le moment (wiki injoignable).</p>}
          <ul className="cmp-touches">
            {c.elements.map((e) => {
              const n = reunis[e.nom] ?? 0;
              return (
                <li key={e.nom} className={n >= e.quantite ? 'is-ok' : undefined}>
                  <span className="cmp-ligne">
                    {(() => { const img = imageDe(e.nom, detail?.elements?.find((x) => x.nom === e.nom)?.image); return img ? <button type="button" className="cmp-wk-zoom" onClick={() => onImage({ src: img, titre: nomFr(e.nom) })} aria-label={`Voir ${nomFr(e.nom)} en grand`}><img className="cmp-wk-vignette" src={img} alt="" loading="lazy" /></button> : null; })()}
                    <span title={e.nom}>{nomFr(e.nom)}</span>
                    <BoutonProposer nom={e.nom} />
                  </span>
                  <span className="cmp-ligne">
                    <InputText aria-label={`Réunis : ${e.nom}`} inputMode="numeric" style={{ width: 60 }} value={n || ''} placeholder="0"
                      onChange={(ev) => onReunir(c.uuid, e.nom, Number(ev.target.value.replace(/\D/g, '')) || 0)} />
                    <span className="cmp-mono cmp-small">/ {e.quantite}</span>
                  </span>
                </li>
              );
            })}
          </ul>
          <p className="cmp-muted cmp-small">{c.reputation ? `Réputation Wikelo : +${c.reputation} · ` : ''}<span className="cmp-mono">{c.uuid.slice(0, 8)}</span></p>
        </>
      )}
      </div>
    </li>
  );
}

function Wikelo() {
  const [d, setD] = useState(null);
  const [suivi, setSuivi] = useState({ favoris: [], reunis: {} });
  const [cat, setCat] = useState('tous');
  const [q, setQ] = useState('');
  const [indispo, setIndispo] = useState(false);
  const [erreur, setErreur] = useState(null);
  const [image, setImage] = useState(null);
  const [portail, setPortail] = useState(null); // null = en cours, false = indisponible, Map = images du portail
  const [envoi, setEnvoi] = useState(null);
  useEffect(() => {
    apiPortail('GET', 'wikelo').then((p) => setPortail(p?.contracts?.length ? indexerWikeloPortail(p) : false)).catch(() => setPortail(false));
    invoke('wikelo_liste', {}).then(setD).catch((e) => setErreur(String(e)));
    invoke('wikelo_suivi').then(setSuivi).catch(() => {});
  }, []);
  const onFavori = (uuid, favori) => invoke('wikelo_favori', { uuid, favori }).then(setSuivi).catch((e) => setErreur(String(e)));
  const onProposer = async ({ uuid, nom }) => {
    const chemin = await invoke('wikelo_choisir_image').catch(() => null);
    if (!chemin) return;
    setEnvoi({ ton: 'info', texte: `Envoi de l’image pour « ${nom} »…` });
    invoke('wikelo_proposer', { uuid, image: chemin, note: null })
      .then(() => setEnvoi({ ton: 'success', texte: `Merci ! Ton image pour « ${nom} » est envoyée : elle apparaîtra pour tout le CES une fois validée par la modération.` }))
      .catch((e) => setEnvoi({ ton: 'danger', texte: String(e) }));
  };
  const onReunir = (uuid, objet, quantite) => invoke('wikelo_reunir', { uuid, objet, quantite }).then(setSuivi).catch((e) => setErreur(String(e)));

  if (erreur && !d) return <Message severity="danger">{erreur}</Message>;
  if (!d) return <p className="cmp-muted">Chargement des contrats de Wikelo…</p>;
  const texte = q.trim().toLowerCase();
  const liste = d.contrats.filter((c) =>
    (indispo || c.disponible)
    && (cat === 'tous' || (cat === 'favoris' ? suivi.favoris?.includes(c.uuid) : c.categorie === cat))
    && (!texte || c.titre.toLowerCase().includes(texte) || c.elements.some((e) => e.nom.toLowerCase().includes(texte))));

  return (
    <div className="cmp-stack">
      <Panel header="Emporium de Wikelo">
        <p className="cmp-muted">Ce que Wikelo demande pour chaque échange. Note ce que tu as déjà réuni : c’est gardé sur ce PC.</p>
        <div className="cmp-wk-cats cmp-gap"><SegmentedControl ariaLabel="Catégorie" value={cat} onChange={setCat} options={CATEGORIES_WK} /></div>
        <div className="cmp-gap"><InputText aria-label="Chercher un contrat ou un objet" placeholder="Chercher un contrat ou un objet…" value={q} onChange={(e) => setQ(e.target.value)} /></div>
        <div className="cmp-gap"><Checkbox label="Afficher aussi les contrats indisponibles en jeu" checked={indispo} onChange={(e) => setIndispo(e.target.checked)} /></div>
      </Panel>
      {envoi && <Message severity={envoi.ton} onClose={() => setEnvoi(null)}>{envoi.texte}</Message>}
      {liste.length === 0
        ? <EmptyState icon="search" title="Aucun contrat" text={cat === 'favoris' ? 'Suis un contrat avec l’étoile pour le retrouver ici.' : 'Essaie une autre catégorie ou une autre recherche.'} />
        : <ul className="wkc-grille">{liste.map((c) => <ContratWikelo key={c.uuid} c={c} suivi={suivi} onFavori={onFavori} onReunir={onReunir} onImage={setImage}
            pw={portail ? portail.get(c.uuid) : null} portailPret={portail === false} onProposer={portail ? onProposer : null} />)}</ul>}
      <Visionneuse image={image} onFermer={() => setImage(null)} />
      <p className="cmp-muted cmp-small">
        Contrats : <strong>api.star-citizen.wiki</strong> (Star Citizen Wiki){d.version ? `, jeu ${d.version}` : ''}. Usage non commercial.
      </p>
    </div>
  );
}

const ICONE_ETAPE = {
  'session.start': 'rocket', 'session.end': 'close', 'presence.universe': 'pin', 'presence.menu': 'home',
  'travel.route': 'right', 'travel.quantum': 'ship', 'travel.jurisdiction': 'shield', 'travel.armistice': 'lock',
  'player.down': 'medic', 'vehicle.destroyed': 'sword', 'mission.completed': 'flag',
};
const jourSession = (iso) => new Date(iso).toLocaleDateString('fr-FR', { weekday: 'long', day: 'numeric', month: 'long' });
const heureCourte = (iso) => new Date(iso).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' });

/** Carnet de voyage : sessions de jeu et leurs étapes (gardées sur ce PC). */
function Carnet() {
  const [sessions, setSessions] = useState(null);
  const [ouverte, setOuverte] = useState(null);
  const [etapes, setEtapes] = useState([]);
  const [masquerArmistice, setMasquerArmistice] = useState(true);
  const [retour, setRetour] = useState(null);
  const [occupe, setOccupe] = useState(false);
  const [lieux, setLieux] = useState([]);
  const lire = useCallback(() => {
    invoke('carnet_sessions').then(setSessions).catch(() => setSessions([]));
    invoke('carnet_lieux').then(setLieux).catch(() => setLieux([]));
  }, []);
  useEffect(() => { lire(); const t = setInterval(lire, 15000); return () => clearInterval(t); }, [lire]);
  useEffect(() => {
    if (!ouverte) return undefined;
    const charger = () => invoke('carnet_session', { session: ouverte }).then(setEtapes).catch(() => setEtapes([]));
    charger();
    const t = setInterval(charger, 15000);
    return () => clearInterval(t);
  }, [ouverte]);

  const importer = async () => {
    setOccupe(true);
    try { setRetour({ severity: 'success', texte: await invoke('carnet_importer') }); lire(); }
    catch (e) { setRetour({ severity: 'danger', texte: String(e) }); }
    finally { setOccupe(false); }
  };

  if (!sessions) return <p className="cmp-muted">Chargement du carnet…</p>;
  const visibles = etapes.filter((e) => !(masquerArmistice && e.type === 'travel.armistice'));

  return (
    <div className="cmp-stack">
      {retour && <Message severity={retour.severity}>{retour.texte}</Message>}
      {sessions.length === 0 && (
        <EmptyState icon="file" title="Carnet vide" text="Tes sessions de jeu apparaîtront ici. Tu peux aussi importer celles de tes anciens journaux."
          action={<Button label="Importer l’historique" icon="download" loading={occupe} onClick={importer} />} />
      )}
      {sessions.length > 0 && (
        <div data-visite="carte" className="cmp-visite-cible"><Panel header={ouverte ? `Carte stellaire — session du ${jourSession(ouverte)}` : 'Carte stellaire'}>
          <StarMap visits={ouverte ? lieux.filter((l) => l.session === ouverte) : lieux} showPeriod={!ouverte} height={540} />
        </Panel></div>
      )}
      {sessions.length > 0 && (
        <ul className="cmp-routes">
          {sessions.map((s) => (
            <li key={s.session} className="cmp-route">
              <button type="button" className="cmp-session" onClick={() => setOuverte(ouverte === s.session ? null : s.session)} aria-expanded={ouverte === s.session}>
                <span className="cmp-route__tete">
                  <strong>{jourSession(s.session)}</strong>
                  {!s.terminee && <Tag severity="success" value="en cours" />}
                  <span className="cmp-route__gain cmp-wk-avance">{heureCourte(s.session)} → {heureCourte(s.derniere)}</span>
                </span>
                <span className="cmp-muted cmp-small">
                  {s.sauts} saut(s) · {s.missions} mission(s){s.aTerre ? ` · ${s.aTerre} fois à terre` : ''}{s.vaisseaux ? ` · ${s.vaisseaux} vaisseau(x) détruit(s)` : ''}
                  {s.juridictions.length ? ` · ${s.juridictions.join(', ')}` : ''}
                </span>
              </button>
              {ouverte === s.session && (
                <>
                  <Checkbox label="Masquer les entrées et sorties de zone d’armistice" checked={masquerArmistice} onChange={(e) => setMasquerArmistice(e.target.checked)} />
                  <ul className="cmp-events">
                    {visibles.map((e, i) => (
                      <li key={`${e.at}-${i}`}>
                        <Icon name={ICONE_ETAPE[e.type] ?? 'info'} size={16} />
                        <span className="cmp-events__txt">{e.texte}</span>
                        <time className="cmp-mono">{heureCourte(e.at)}</time>
                      </li>
                    ))}
                  </ul>
                </>
              )}
            </li>
          ))}
        </ul>
      )}
      {sessions.length > 0 && (
        <div className="cmp-actions"><Button label="Importer l’historique (anciens journaux)" variant="text" icon="download" loading={occupe} onClick={importer} /></div>
      )}
      <p className="cmp-muted cmp-small">
        Le carnet est gardé sur ce PC (90 jours). Seuls les types activés dans « Partage » partent au portail : sessions, sauts quantiques (avec leur destination), etc.
      </p>
    </div>
  );
}

function Bientot({ titre }) {
  return <EmptyState icon="time" title={titre} text="Cet écran arrive dans une prochaine étape." />;
}

// --- Thème : Système / Clair / Sombre (point 7) — mémorisé sur ce PC ---------------------------------------
const THEMES = ['systeme', 'clair', 'sombre'];
const THEME_ICONE = { systeme: 'monitor', clair: 'sun', sombre: 'moon' };
const THEME_LIBELLE = { systeme: 'Thème : comme Windows', clair: 'Thème : clair', sombre: 'Thème : sombre' };

function useTheme() {
  const [theme, setTheme] = useState(() => { try { return localStorage.getItem('cmp-theme') || 'systeme'; } catch { return 'systeme'; } });
  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const appliquer = () => {
      const sombre = theme === 'sombre' || (theme === 'systeme' && media.matches);
      document.documentElement.setAttribute('data-mui-color-scheme', sombre ? 'dark' : 'light');
    };
    appliquer();
    try {
      localStorage.setItem('cmp-theme', theme);
      // Même choix pour le site embarqué (même origine, clé du portail `mui-mode` : il suit par l'événement storage).
      localStorage.setItem('mui-mode', { systeme: 'system', clair: 'light', sombre: 'dark' }[theme] || 'system');
    } catch { /* ignoré */ }
    media.addEventListener('change', appliquer);
    return () => media.removeEventListener('change', appliquer);
  }, [theme]);
  const suivant = () => setTheme(THEMES[(THEMES.indexOf(theme) + 1) % THEMES.length]);
  return { theme, suivant };
}

// --- Barre de titre maison (point 6) : la fenêtre n'a plus de cadre Windows ----------------------------------
const fenetre = getCurrentWindow();

function BoutonsFenetre() {
  const [agrandie, setAgrandie] = useState(false);
  useEffect(() => {
    const maj = () => fenetre.isMaximized().then(setAgrandie).catch(() => {});
    maj();
    const fin = fenetre.onResized(maj);
    return () => { fin.then((f) => f()); };
  }, []);
  return (
    <div className="cmp-fenetre">
      <button type="button" aria-label="Réduire" title="Réduire" onClick={() => fenetre.minimize()}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 5.5h8" stroke="currentColor" strokeWidth="1.2" /></svg>
      </button>
      <button type="button" aria-label={agrandie ? 'Restaurer' : 'Agrandir'} title={agrandie ? 'Restaurer' : 'Agrandir'} onClick={() => fenetre.toggleMaximize()}>
        {agrandie
          ? <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M2.5 3.5h4v4h-4zM3.5 3.5V2h4.5v4.5H6.5" fill="none" stroke="currentColor" strokeWidth="1.1" /></svg>
          : <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x="1.5" y="1.5" width="7" height="7" fill="none" stroke="currentColor" strokeWidth="1.1" /></svg>}
      </button>
      <button type="button" className="cmp-fenetre__fermer" aria-label="Fermer (l’application reste dans la zone de notification)" title="Fermer (reste dans la zone de notification)" onClick={() => fenetre.close()}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M1.5 1.5l7 7M8.5 1.5l-7 7" stroke="currentColor" strokeWidth="1.2" /></svg>
      </button>
    </div>
  );
}

/** Écran de connexion obligatoire : l'application est réservée aux membres du CES et aux invités autorisés. */
function Connexion({ compte, relire, icone }) {
  return (
    <main className="cmp-porte">
      <div className="cmp-porte__carte">
        <LogoCompagnon className="cmp-porte__logo" />
        <p className="cmp-muted">Réservé aux membres du CES et aux invités autorisés. Connecte-toi avec ton compte du portail.</p>
        {compte ? <Compte compte={compte} relire={relire} porte /> : <p className="cmp-muted">Chargement…</p>}
        <p className="cmp-porte__marque"><img src={logoCes} alt="" width="22" height="22" />Portail du Corps Expéditionnaire Spatial · {__CES_PORTAIL__.replace('https://', '')}</p>
      </div>
    </main>
  );
}

export default function App() {
  const [ecran, setEcran] = useState('accueil');
  const { etat, erreur } = useEtat();
  const { compte, relire } = useCompte();
  const notifs = useNotifications(compte?.relie);
  const [defcon, setDefcon] = useDefcon(compte?.relie);
  const alerte = defcon && defcon.niveau >= 1 && defcon.niveau <= 3 && (defcon.bandeau || defcon.masque);
  // Élément ouvert dans un écran natif (mission, sujet du forum, profil) ; `null` = la liste.
  const [cibles, setCibles] = useState({});
  const cible = (id) => cibles[id] ?? null;
  const fixerCible = (id) => (v) => setCibles((c) => ({ ...c, [id]: v }));
  const allerModule = (id) => { setEcran(id); setCibles((c) => ({ ...c, [id]: null })); };
  // Clic sur une notification (demande du CES, 22/11) : on n'ouvre que ce qui a un écran dans l'application —
  // SOS, DEFCON, mission, sujet du forum, profil. Le reste est simplement marqué comme lu (plus de page du site).
  const allerNotif = (n) => {
    if (n?.type === 'sos') { setSosOuvert(true); return true; }
    if (n?.type === 'defcon') { setEcran('defcon'); return true; }
    const natif = ecranDeLien(n?.url);
    if (!natif || natif[0] === 'accueil') return false;
    setEcran(natif[0]); setCibles((c) => ({ ...c, [natif[0]]: natif[1] }));
    return true;
  };
  // Menus et tiroirs de l'application : la webview du site (native) passerait par-dessus. On la masque pendant ce temps.
  const [tiroir, setTiroir] = useState(false);
  const [menuOuvert, setMenuOuvert] = useState(false);
  const [visite, setVisite] = useState(false);
  const [sosOuvert, setSosOuvert] = useState(false);
  const [radioOuverte, setRadioOuverte] = useState(false);
  const fermerRadio = useCallback(() => setRadioOuverte(false), []);
  const [navPos, setNavPosEtat] = useState(() => { try { return localStorage.getItem('cmp-nav') || 'gauche'; } catch { return 'gauche'; } });
  const setNavPos = (v) => { setNavPosEtat(v); try { localStorage.setItem('cmp-nav', v); } catch { /* ignoré */ } };
  const aller = (id, cibleId = null) => { setEcran(id); setCibles((c) => ({ ...c, [id]: cibleId })); };
  const fermerSos = useCallback(() => setSosOuvert(false), []);
  const couvrant = tiroir || menuOuvert || visite || sosOuvert;
  const [fond, setFond, fondPerso] = useFond();
  const moi = compte?.utilisateur?.username;
  const sos = useSos(compte?.relie, moi);
  // Visite proposée une fois, à la première connexion (mémorisé dans ce navigateur intégré).
  useEffect(() => {
    if (!compte?.relie) return undefined;
    let deja = false;
    try { deja = localStorage.getItem('cmp-visite-v1') === 'vue'; } catch { /* ignoré */ }
    if (deja) return undefined;
    const t = setTimeout(() => setVisite(true), 1200);
    return () => clearTimeout(t);
  }, [compte?.relie]);
  const fermerVisite = useCallback(() => { setVisite(false); try { localStorage.setItem('cmp-visite-v1', 'vue'); } catch { /* ignoré */ } }, []);
  const [canal, setCanal] = useState(null);
  const { theme, suivant } = useTheme();
  const [testeur, setTesteur] = useState(() => { try { return localStorage.getItem('cmp-testeur') === '1'; } catch { return false; } });
  const [clics, setClics] = useState(0);
  useEffect(() => { invoke('canal_etat').then(setCanal).catch(() => {}); }, []);
  // Interface rechargée : la webview du site, posée par-dessus, ne doit pas rester affichée.
  useEffect(() => { invoke('site_masquer').catch(() => {}); }, []);
  const clicVersion = () => {
    const n = clics + 1;
    setClics(n);
    if (n >= 5 && !testeur) {
      setTesteur(true);
      try { localStorage.setItem('cmp-testeur', '1'); } catch { /* ignoré */ }
      setEcran('outils');
    }
  };
  const estCanary = (canal?.actif ?? 'canary') === 'canary';
  const deplacer = (e) => {
    if (e.button !== 0 || e.target.closest('button, a, input, select, textarea, .cmp-pastille')) return;
    if (e.detail === 2) fenetre.toggleMaximize();
    else fenetre.startDragging();
  };
  const p = PRESENCE[etat?.presence] ?? PRESENCE.offline;
  const classes = ['cmp', 'cmp-hud', alerte ? `cmp--defcon cmp--defcon-${defcon.niveau}` : '', fond.src ? '' : 'cmp-hud--sans-fond'].filter(Boolean).join(' ');
  return (
    <DsRoot className={classes} style={fond.src ? { '--hud-fond': `url(${fond.src})` } : undefined}>
      {/* Visière : toute la barre déplace la fenêtre (hors boutons) ; double-clic = agrandir / restaurer. */}
      <header className="cmp-head hud-visiere" onMouseDown={deplacer}>
        <img src={canal?.build === 'stable' ? iconeStable : iconeCanary} alt="" width="26" height="26" draggable={false} />
        <div className="cmp-title">
          <strong>CES Compagnon</strong>
          <small className="hud-sous-titre">Poste de bord · {compte?.relie ? (moi || 'connecté') : 'non connecté'}</small>
        </div>
        {compte?.relie && etat && <span className="hud-puce" title="État du jeu"><Status label={p.label} state={p.state} /></span>}
        <Horloge />
        {compte?.relie && (
          <span className="hud-actions" data-visite="sos">
            <BoutonRsi />
            <BoutonSos onOuvrir={() => setSosOuvert(true)} nbActifs={sos.actives.length} mien={!!sos.mien} />
          </span>
        )}
        <span className={estCanary ? 'cmp-pastille cmp-pastille--canary' : 'cmp-pastille'} onClick={clicVersion} title={`CES Compagnon ${__APP_VERSION__}`}>
          {estCanary ? `canary · ${__APP_VERSION__}` : `v${__APP_VERSION__}`}
        </span>
        {compte?.relie && (
          <button type="button" data-visite="cloche" className={notifs.nonLues ? 'cmp-theme cmp-cloche has-unread' : 'cmp-theme cmp-cloche'}
            aria-label={notifs.nonLues ? `Notifications, ${notifs.nonLues} non lues` : 'Notifications'} title="Notifications"
            onClick={() => setEcran('notifications')}>
            <Icon name="bell" size={16} />
            {notifs.nonLues > 0 && <i className="cmp-cloche__point" aria-hidden="true" />}
          </button>
        )}
        <button type="button" className="cmp-theme" aria-label={THEME_LIBELLE[theme]} title={THEME_LIBELLE[theme]} onClick={suivant}>
          <Icon name={THEME_ICONE[theme]} size={16} />
        </button>
        <BoutonsFenetre />
      </header>
      {/* DEFCON 3 à 1 : bandeau DS-CES-10 sous la visière (place réservée dans le flux : le site embarqué
          descend d'autant) ; le liseré autour de la fenêtre est en CSS (.cmp--defcon). */}
      {alerte && defcon.bandeau && (
        <>
          <div className="cmp-defcon-place" aria-hidden="true" />
          <DefconBar active level={defcon.niveau} message={defcon.message || undefined} height={48}
            onClose={() => invoke('bandeau_masquer').then(() => setDefcon({ ...defcon, bandeau: false, masque: true })).catch(() => {})} />
        </>
      )}

      {!compte?.relie ? <Connexion compte={compte} relire={relire} icone={canal?.build === 'stable' ? iconeStable : iconeCanary} /> : (
      <div className={`hud-corps hud-corps--${navPos}`}>
        <nav className="hud-rail" aria-label="Navigation">
          <div className="hud-rail__defil">
            <span className="hud-rail__groupe" data-visite="nav-app">
              {ECRANS.filter((e) => e.barre).map((e) => (
                <button key={e.id} type="button" className={ecran === e.id ? 'hud-rail__lien is-actif' : 'hud-rail__lien'} aria-current={ecran === e.id ? 'page' : undefined}
                  title={e.label} onClick={() => setEcran(e.id)}>
                  <Icon name={e.icon} size={19} /><span className="hud-rail__txt">{e.label}</span>
                </button>
              ))}
            </span>
            <span className="hud-rail__sep" aria-hidden="true" />
            <span className="hud-rail__groupe" data-visite="nav-portail">
              {modulesPortail(compte?.moi).map((e) => (
                <button key={e.id} type="button" className={ecran === e.id ? 'hud-rail__lien is-actif' : 'hud-rail__lien'} aria-current={ecran === e.id ? 'page' : undefined}
                  title={e.label} onClick={() => allerModule(e.id)}>
                  <Icon name={e.icon} size={19} /><span className="hud-rail__txt">{e.label}</span>
                </button>
              ))}
            </span>
          </div>
          <VinyleNav onOuvrir={() => setRadioOuverte(true)} compact={navPos !== 'gauche'} />
          <div data-visite="compte" className="hud-rail__compte">
            <MenuCompte libelle={moi || 'Compte'} onEtat={setMenuOuvert} actif={ECRANS.some((e) => !e.barre && e.id === ecran)}
              entrees={[...ECRANS.filter((e) => !e.barre).map((e) => ({ label: e.label, icon: e.icon, onSelect: () => setEcran(e.id) })),
                { label: 'Visite guidée', icon: 'sparkle', onSelect: () => setVisite(true) }]} />
          </div>
        </nav>

        <VisiteGuidee ouverte={visite} onFermer={fermerVisite} allerEcran={(id) => setEcran(id)} />
        <main className="cmp-main">
          {PORTAIL.some((e) => e.id === ecran)
            ? (() => {
              const e = PORTAIL.find((x) => x.id === ecran);
              const moiId = compte?.moi?.id;
              return (
                <div className={`cmp-page cmp-page--${e.id}`}>
                  {!cible(e.id) && <ModuleHero eyebrow={e.eyebrow} title={e.titre} desc={e.desc} icon={e.icon} />}
                  {e.id === 'missions' && <Missions moiId={moiId} cible={cible('missions')} setCible={fixerCible('missions')} />}
                  {e.id === 'forum' && <Forum cible={cible('forum')} setCible={fixerCible('forum')} />}
                  {e.id === 'cesoir' && <CeSoir moi={moi} />}
                  {e.id === 'contrats' && <Contrats cible={cible('contrats')} setCible={fixerCible('contrats')} />}
                  {e.id === 'annuaire' && <Annuaire cible={cible('annuaire')} setCible={fixerCible('annuaire')} />}
                  {e.id === 'defcon' && <Defcon peutGerer={peutDefcon(compte?.moi)} />}
                  {e.id === 'mesdonnees' && <MesDonnees moiId={moiId} />}
                </div>
              );
            })()
            : (() => {
              const e = ECRANS.find((x) => x.id === ecran) || ECRANS[0];
              return (
                <div className={`cmp-page cmp-page--${e.id}`}>
                  {ecran !== 'accueil' && <ModuleHero eyebrow={e.eyebrow} title={e.titre} desc={e.desc} icon={e.icon} stats={statsEcran(e.id, etat, compte)} />}
                  <MiseAJour />
                  {ecran === 'accueil' && <Accueil etat={etat} erreur={erreur} compte={compte} allerCompte={() => setEcran('compte')} sos={sos} ouvrirSos={() => setSosOuvert(true)} defcon={defcon} aller={aller} ouvrirRadio={() => setRadioOuverte(true)} />}
                  {ecran === 'carnet' && <Carnet />}
                  {ecran === 'notifications' && <Notifications onOuvrir={allerNotif} onChange={notifs.fixer} />}
                  {ecran === 'personnages' && <Personnages jeuLance={!!etat?.jeuLance} />}
                  {ecran === 'compte' && <Compte compte={compte} relire={relire} />}
                  {ecran === 'partage' && <Partage compte={compte} allerCompte={() => setEcran('compte')} />}
                  {ecran === 'commerce' && <><JetonUex /><Commerce /></>}
                  {ecran === 'minage' && <Minage compte={compte} />}
                  {ecran === 'wikelo' && <Wikelo />}
                  {ecran === 'apropos' && <APropos canal={canal} />}
                  {ecran === 'outils' && <Outils etat={etat} compte={compte} allerCompte={() => setEcran('compte')} testeur={testeur} canal={canal} setCanal={setCanal} fond={fond} setFond={setFond} fondPerso={fondPerso} nav={navPos} setNav={setNavPos} />}
                </div>
              );
            })()}
        </main>
        {radioOuverte && (
          <FenetreFlottante cle="radio" titre="Radio du CES" icone={<RiMusic2Line size={15} />} onFermer={fermerRadio} largeur={380}>
            <PanneauRadio />
          </FenetreFlottante>
        )}
        {sosOuvert && (
          <FenetreFlottante cle="sos" titre="SOS — Corps Expéditionnaire Spatial" icone="alert" ton="danger" onFermer={fermerSos} largeur={440}>
            <PanneauSos sos={sos} moi={moi} />
          </FenetreFlottante>
        )}
      </div>
      )}

      {compte?.relie && (
        <MenuContextuel actions={[
          { label: 'Accueil', icon: 'home', f: () => setEcran('accueil') },
          { label: 'Notifications', icon: 'bell', f: () => setEcran('notifications') },
          { label: 'Radio : lecture / pause', icone: <RiMusic2Line size={15} />, f: () => radio.basculer() },
          { label: 'Fenêtre SOS', icon: 'alert', f: () => setSosOuvert(true) },
          'sep',
          { label: 'Réglages', icon: 'settings', f: () => setEcran('outils') },
        ]} />
      )}
    </DsRoot>
  );
}
