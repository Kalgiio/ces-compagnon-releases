// Accueil « passerelle » (demandes du CES, 23–24/11 : ambiance « console tactique », puis « un mélange entre passerelle
// et tuiles ») : un bandeau de lectures fixe en haut, puis les blocs de la passerelle en TUILES déplaçables,
// redimensionnables et masquables (TableauDeBord). Chaque bloc est un composant indépendant.
import React, { useEffect, useState } from 'react';
import { Icon, Button } from '@aurore/components';
import { RiMusic2Line } from '@remixicon/react';
import { api, Avatar, media } from './Natif.jsx';
import { useRadio, radio } from './Radio.jsx';

const duree = (s) => { const h = Math.floor((s || 0) / 3600); const m = Math.round(((s || 0) % 3600) / 60); return h ? `${h} h ${String(m).padStart(2, '0')}` : `${m} min`; };
const dans = (iso) => {
  const d = (new Date(iso) - Date.now()) / 1000;
  if (d <= 0) return 'en cours';
  const j = Math.floor(d / 86400); const h = Math.floor((d % 86400) / 3600); const m = Math.floor((d % 3600) / 60);
  return j ? `J-${j} · ${h} h` : h ? `${h} h ${String(m).padStart(2, '0')}` : `${m} min`;
};

/** Donnée du portail relue régulièrement (partagée entre les blocs par un petit cache). */
const cache = new Map();
export function usePortail(chemin, toutesLes = 60000) {
  const [v, setV] = useState(() => cache.get(chemin) ?? null);
  useEffect(() => {
    let actif = true;
    const lire = () => api('GET', chemin).then((x) => { cache.set(chemin, x); if (actif) setV(x); }).catch(() => actif && setV((p) => p ?? { erreur: true }));
    lire();
    const t = setInterval(lire, toutesLes);
    return () => { actif = false; clearInterval(t); };
  }, [chemin, toutesLes]);
  return v;
}

/** Bloc de passerelle : en-tête monospace (poignée de la tuile), corps qui défile. */
export function Bloc({ titre, icone, action, children, className }) {
  return (
    <section className={`psl-bloc${className ? ` ${className}` : ''}`}>
      <header className="psl-bloc__tete"><span>{typeof icone === 'string' ? <Icon name={icone} size={13} /> : icone} {titre}</span>{action}</header>
      <div className="psl-bloc__corps">{children}</div>
    </section>
  );
}

function Lecture({ etiquette, valeur, ton }) {
  return <div className={`psl-lecture${ton ? ` psl-lecture--${ton}` : ''}`}><span>{etiquette}</span><strong>{valeur}</strong></div>;
}

const prochainesOps = (events) => {
  const maintenant = Date.now();
  return Array.isArray(events) ? events.filter((m) => m.status === 'planifie' && new Date(m.dateEnd || m.dateStart).getTime() + 4 * 3600e3 > maintenant) : [];
};

/** Bandeau fixe : état du jeu, version, DEFCON, SOS, membres en jeu, heures du CES. */
export function BandeauPasserelle({ etat, defcon, sos }) {
  const live = usePortail('companion/live', 60000);
  const niveau = defcon?.niveau && defcon.niveau <= 5 ? defcon.niveau : 5;
  const presence = etat?.presence === 'universe' ? 'Dans le Verse' : etat?.presence === 'menu' ? 'Au menu' : etat?.presence === 'launching' ? 'Lancement' : 'Hors ligne';
  const enJeu = live?.playing || [];
  return (
    <div className="psl-bandeau">
      <Lecture etiquette="Star Citizen" valeur={presence} ton={etat?.presence && etat.presence !== 'offline' ? 'ok' : undefined} />
      <Lecture etiquette="Version" valeur={etat?.version ? etat.version.split('.').slice(0, 3).join('.') : '—'} />
      <Lecture etiquette="DEFCON" valeur={niveau} ton={niveau <= 2 ? 'danger' : niveau === 3 ? 'warn' : undefined} />
      <Lecture etiquette="SOS ouverts" valeur={sos.actives.length} ton={sos.actives.length ? 'danger' : undefined} />
      <Lecture etiquette="En jeu (CES)" valeur={enJeu.length} ton={enJeu.length ? 'ok' : undefined} />
      <Lecture etiquette="CES · 7 jours" valeur={live?.week ? duree(live.week.weekSec) : '—'} />
    </div>
  );
}

export function BlocOperation({ aller, moiId }) {
  const events = usePortail('events', 120000);
  const prochaines = prochainesOps(events);
  const op = prochaines[0];
  const inscrits = op ? (op.EventParticipants || []).filter((p) => p.status === 'inscrit').length : 0;
  const moiInscrit = op && (op.EventParticipants || []).some((p) => (p.UserId === moiId || p.User?.id === moiId) && p.status !== 'annule');
  return (
    <Bloc titre="Prochaine opération" icone="rocket" action={<button type="button" className="psl-lien" onClick={() => aller('missions')}>Toutes</button>}>
      {!events ? <p className="cmp-muted">Chargement…</p> : !op ? <p className="cmp-muted">Aucune opération prévue.</p> : (
        <button type="button" className="psl-op__carte" onClick={() => aller('missions', op.id)} style={op.banner ? { '--psl-img': `url(${media(op.banner)})` } : undefined}>
          <span className="psl-op__compte">{dans(op.dateStart)}</span>
          <strong>{op.title}</strong>
          <span>{new Date(op.dateStart).toLocaleString('fr-FR', { weekday: 'long', day: 'numeric', month: 'long', hour: '2-digit', minute: '2-digit' })}</span>
          <span className="psl-op__pied">{op.location && <span><Icon name="pin" size={12} /> {op.location}</span>}<span><Icon name="users" size={12} /> {inscrits} inscrit{inscrits > 1 ? 's' : ''}</span>{moiInscrit && <span className="psl-ok">Tu es inscrit</span>}</span>
        </button>
      )}
      {prochaines.length > 1 && (
        <ul className="psl-liste">{prochaines.slice(1, 5).map((x) => (
          <li key={x.id}><button type="button" onClick={() => aller('missions', x.id)}><span>{x.title}</span><time>{dans(x.dateStart)}</time></button></li>
        ))}</ul>
      )}
    </Bloc>
  );
}

export function BlocAlerte({ defcon, sos, aller, ouvrirSos }) {
  const niveau = defcon?.niveau && defcon.niveau <= 5 ? defcon.niveau : 5;
  return (
    <Bloc titre="Alerte et SOS" icone="alert" action={<button type="button" className="psl-lien" onClick={() => aller('defcon')}>DEFCON</button>}>
      <div className={`psl-defcon psl-defcon--${niveau}`}><strong>{niveau}</strong><span>{defcon?.message || (niveau === 5 ? 'Aucune alerte' : 'Alerte en cours')}</span></div>
      {sos.actives.length === 0 ? <p className="cmp-muted cmp-small">Aucun SOS en cours.</p> : (
        <ul className="psl-liste">{sos.actives.slice(0, 4).map((a) => (
          <li key={a.id}><button type="button" onClick={ouvrirSos}><span className="psl-danger">{a.kindEmoji} {a.name}</span><time>{(a.responders || []).length} en route</time></button></li>
        ))}</ul>
      )}
      <Button size="sm" variant="outlined" icon="alert" label="Fenêtre SOS" onClick={ouvrirSos} />
    </Bloc>
  );
}

export function BlocEnJeu() {
  const live = usePortail('companion/live', 60000);
  const enJeu = live?.playing || [];
  return (
    <Bloc titre="En jeu maintenant" icone="users">
      {!live ? <p className="cmp-muted">Chargement…</p> : enJeu.length === 0 ? <p className="cmp-muted cmp-small">Personne en jeu pour l’instant.</p> : (
        <ul className="psl-membres">{enJeu.slice(0, 12).map((p) => {
          const u = p.user || p;
          return <li key={u.id || u.username}><Avatar u={u} taille={22} /><span>{u.username}</span><small>{p.state === 'universe' ? 'Verse' : 'Menu'}</small></li>;
        })}</ul>
      )}
    </Bloc>
  );
}

export function BlocCeSoir({ aller }) {
  const soirees = usePortail('soirees', 120000);
  const ceSoir = (soirees?.soirees || []).filter((s) => s.status !== 'cancelled').slice(0, 4);
  return (
    <Bloc titre="Ce soir" icone="calendar" action={<button type="button" className="psl-lien" onClick={() => aller('cesoir')}>Ouvrir</button>}>
      {ceSoir.length === 0 ? <p className="cmp-muted cmp-small">Rien de prévu. <button type="button" className="psl-lien" onClick={() => aller('cesoir')}>Proposer une soirée</button></p> : (
        <ul className="psl-liste">{ceSoir.map((s) => (
          <li key={s.id}><button type="button" onClick={() => aller('cesoir')}><span>{s.activity}</span><time>{new Date(s.startsAt).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' })} · {s.yes} ✓</time></button></li>
        ))}</ul>
      )}
    </Bloc>
  );
}

export function BlocRadio({ ouvrirRadio }) {
  const r = useRadio();
  const m = r.morceau || r.live?.track;
  return (
    <Bloc titre="Radio du CES" icone={<RiMusic2Line size={13} />} action={<button type="button" className="psl-lien" onClick={ouvrirRadio}>Lecteur</button>}>
      <div className="psl-radio">
        <button type="button" className="psl-radio__btn" onClick={radio.basculer} aria-label={r.enLecture ? 'Pause' : 'Écouter le direct'}>{r.enLecture ? 'II' : '▶'}</button>
        <span><strong>{m?.title || 'Antenne du CES'}</strong><small className="cmp-muted">{r.enLecture ? 'En écoute' : 'En pause'}{m?.artist ? ` · ${m.artist}` : ''}</small></span>
      </div>
    </Bloc>
  );
}

export function BlocOutils({ aller }) {
  return (
    <Bloc titre="Raccourcis" icone="settings">
      <div className="psl-outils">
        {[['commerce', 'chart', 'Commerce'], ['minage', 'ship', 'Minage'], ['wikelo', 'star', 'Wikelo'], ['contrats', 'file', 'Contrats'], ['carnet', 'book', 'Carnet'], ['personnages', 'user', 'Personnages']].map(([id, ic, l]) => (
          <button key={id} type="button" onClick={() => aller(id)}><Icon name={ic} size={16} /><span>{l}</span></button>
        ))}
      </div>
    </Bloc>
  );
}
