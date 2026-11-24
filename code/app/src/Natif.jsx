// Écrans natifs du portail dans l'application (demande du CES, 21/11) : Missions, Forum, Annuaire, Mes données et
// DEFCON. Même API que le portail, relayée par le cœur Rust (`portail_api`, liste fermée de routes) ; le portail
// décide toujours des droits. Le hub, le planning, la gestion, le bot, l'académie et le garage restent sur le site.
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { SAFE_REHYPE } from '@portail/utils/safeMarkdown';
import { Icon, Button, Message, Panel, Tag, EmptyState, InputText, Select } from '@aurore/components';
import './natif.css';

const MEDIA = 'http://cesmedia.localhost/';
export const api = (methode, chemin, corps) => invoke('portail_api', { methode, chemin, corps: corps ?? null });

/** URL d'image du portail → relais `cesmedia` (images privées) ; les URL absolues (Discord) passent telles quelles. */
export const media = (u) => {
  if (!u) return null;
  if (/^https?:\/\//.test(u)) return u;
  return MEDIA + String(u).replace(/^\/?(media\/)?/, '');
};
const avatarDe = (u) => media(u?.customAvatar) || u?.avatar || null;
const dateLongue = (iso) => (iso ? new Date(iso).toLocaleString('fr-FR', { weekday: 'long', day: 'numeric', month: 'long', hour: '2-digit', minute: '2-digit' }) : '');
const dateCourte = (iso) => (iso ? new Date(iso).toLocaleDateString('fr-FR', { day: 'numeric', month: 'short', year: 'numeric' }) : '');
const relatif = (iso) => {
  if (!iso) return '';
  const d = (new Date(iso) - Date.now()) / 1000;
  const a = Math.abs(d);
  const t = a < 3600 ? `${Math.round(a / 60)} min` : a < 86400 ? `${Math.round(a / 3600)} h` : `${Math.round(a / 86400)} j`;
  return d >= 0 ? `dans ${t}` : `il y a ${t}`;
};
const ouvrirPortail = (chemin) => invoke('site_ouvrir_lien', { url: `${__CES_PORTAIL__}${chemin}` }).catch(() => {});

/** Avatar du membre ; si l'image ne charge pas (lien Discord périmé, image privée…), ses initiales à la place. */
export function Avatar({ u, taille = 32 }) {
  const src = avatarDe(u);
  const [rate, setRate] = useState(false);
  useEffect(() => setRate(false), [src]);
  const nom = String(u?.username || '?').trim();
  const initiales = nom.split(/[\s._-]+/).filter(Boolean).slice(0, 2).map((m) => m[0]).join('').toUpperCase() || '?';
  return src && !rate
    ? <img className="nat-avatar" src={src} alt="" width={taille} height={taille} loading="lazy" onError={() => setRate(true)} />
    : <span className="nat-avatar nat-avatar--vide" style={{ width: taille, height: taille, fontSize: Math.max(9, Math.round(taille * 0.36)) }}>{initiales}</span>;
}

/** Charge une donnée ; `relire()` pour recharger. */
function useDonnee(fn, deps) {
  const [etat, setEtat] = useState({ data: null, erreur: null, charge: true });
  const relire = useCallback(() => {
    setEtat((e) => ({ ...e, charge: true }));
    return fn().then((data) => setEtat({ data, erreur: null, charge: false })).catch((e) => setEtat({ data: null, erreur: String(e), charge: false }));
  }, deps); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { relire(); }, [relire]);
  return { ...etat, relire };
}

function Chargement({ etat, children }) {
  if (etat.erreur) return <Message severity="danger" title="Lecture impossible">{etat.erreur}</Message>;
  if (etat.charge && !etat.data) return <p className="cmp-muted">Chargement…</p>;
  return children;
}

function Markdown({ texte, html }) {
  const composants = { img: ({ src, alt }) => <img src={media(src)} alt={alt || ''} loading="lazy" />, a: ({ href, children }) => <a href={href} onClick={(e) => { e.preventDefault(); invoke('site_ouvrir_lien', { url: href }).catch(() => {}); }}>{children}</a> };
  return <div className="nat-md"><ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={html ? SAFE_REHYPE : []} components={composants}>{texte || ''}</ReactMarkdown></div>;
}

function BarreRetour({ onRetour, libelle, actions }) {
  return (
    <div className="nat-barre">
      <Button size="sm" variant="text" icon="left" label={libelle} onClick={onRetour} />
      <span className="nat-barre__actions">{actions}</span>
    </div>
  );
}

// =============================================================================================== Missions
const STATUT_MISSION = { planifie: { label: 'Planifiée', sev: 'info' }, termine: { label: 'Terminée', sev: 'secondary' }, annule: { label: 'Annulée', sev: 'danger' } };

/** « Aujourd'hui », « Demain », « Cette semaine », « Plus tard » (jours du calendrier, pas des tranches de 24 h). */
function groupesEcheance(liste) {
  const debutJour = (d) => { const x = new Date(d); x.setHours(0, 0, 0, 0); return x.getTime(); };
  const auj = debutJour(Date.now());
  const g = { 'Aujourd’hui': [], Demain: [], 'Cette semaine': [], 'Plus tard': [] };
  liste.forEach((m) => {
    const j = Math.round((debutJour(m.dateStart) - auj) / 86400000);
    (j <= 0 ? g['Aujourd’hui'] : j === 1 ? g.Demain : j < 7 ? g['Cette semaine'] : g['Plus tard']).push(m);
  });
  return Object.entries(g).filter(([, l]) => l.length);
}

function CarteMission({ m, moiId, onOuvrir }) {
  const inscrits = (m.EventParticipants || []).filter((p) => p.status === 'inscrit').length;
  const moi = (m.EventParticipants || []).find((p) => p.UserId === moiId || p.User?.id === moiId);
  return (
    <button type="button" className={`nat-mission${moi && moi.status === 'inscrit' ? ' is-inscrit' : ''}`} onClick={onOuvrir}>
      <span className="nat-mission__img" style={m.banner ? { backgroundImage: `url(${media(m.banner)})` } : undefined}>
        {m.status === 'planifie' && new Date(m.dateStart) > Date.now() && <span className="nat-mission__compte">{relatif(m.dateStart)}</span>}
      </span>
      <span className="nat-mission__corps">
        <span className="nat-mission__haut">
          {m.type && <Tag value={m.type} />}
          {!m.isPublic && <Tag value="CES" severity="secondary" />}
          {moi && <Tag value={moi.status === 'inscrit' ? 'Inscrit' : moi.status === 'reserve' ? 'Réserve' : 'Désisté'} severity={moi.status === 'inscrit' ? 'success' : 'warn'} />}
        </span>
        <strong>{m.title}</strong>
        <span className="cmp-muted cmp-small">{dateLongue(m.dateStart)} · {relatif(m.dateStart)}</span>
        <span className="nat-mission__pied cmp-small">
          {m.location && <span><Icon name="pin" size={13} /> {m.location}</span>}
          <span><Icon name="users" size={13} /> {inscrits} inscrit{inscrits > 1 ? 's' : ''}</span>
        </span>
      </span>
    </button>
  );
}

function DetailMission({ id, moiId, onRetour }) {
  const etat = useDonnee(() => api('GET', `events/${id}`), [id]);
  const [role, setRole] = useState('');
  const [occupe, setOccupe] = useState(false);
  const [retour, setRetour] = useState(null);
  const m = etat.data;
  const postes = useMemo(() => (Array.isArray(m?.roleSlots) ? m.roleSlots.filter((r) => r?.name) : []), [m]);
  const participants = m?.EventParticipants || [];
  const moi = participants.find((p) => p.UserId === moiId || p.User?.id === moiId);
  useEffect(() => { if (moi?.role) setRole(moi.role); }, [moi?.role]);
  const agir = async (fn, ok) => {
    setOccupe(true); setRetour(null);
    try { const r = await fn(); setRetour({ severity: r?.waitlisted ? 'warn' : 'success', texte: r?.waitlisted ? 'Poste complet : tu es en liste d’attente.' : ok }); await etat.relire(); }
    catch (e) { setRetour({ severity: 'danger', texte: String(e) }); }
    finally { setOccupe(false); }
  };
  const sInscrire = (status) => agir(() => api('POST', `events/${id}/join`, { role: role || undefined, status }), status === 'reserve' ? 'Inscrit en réserve.' : 'Inscription enregistrée.');
  const seDesister = () => agir(() => api('DELETE', `events/participations/${moi.id}`), 'Tu n’es plus inscrit à cette opération.');
  const parPoste = participants.reduce((a, p) => { const k = p.role || 'Sans poste'; (a[k] ||= []).push(p); return a; }, {});
  const ouverte = m && m.status === 'planifie';
  return (
    <div className="cmp-stack">
      <BarreRetour onRetour={onRetour} libelle="Toutes les opérations" actions={<Button size="sm" variant="text" icon="link" label="Sur le portail" onClick={() => ouvrirPortail(`/event/${id}`)} />} />
      <Chargement etat={etat}>
        {m && (<>
          <section className="nat-hero" style={m.banner ? { '--nat-img': `url(${media(m.banner)})` } : undefined}>
            <div className="nat-hero__txt">
              <span className="nat-hero__tags">{m.type && <Tag value={m.type} />}<Tag value={STATUT_MISSION[m.status]?.label || m.status} severity={STATUT_MISSION[m.status]?.sev} /></span>
              <h2>{m.title}</h2>
              <p><Icon name="calendar" size={14} /> {dateLongue(m.dateStart)}{m.dateEnd ? ` → ${new Date(m.dateEnd).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' })}` : ''} · {relatif(m.dateStart)}</p>
              {m.location && <p><Icon name="pin" size={14} /> {m.location}</p>}
              {m.Creator && <p className="nat-hero__auteur"><Avatar u={m.Creator} taille={20} /> Commandement : {m.Creator.username}</p>}
            </div>
          </section>
          <div className="nat-deux">
            <Panel header="Briefing">{m.description ? <Markdown texte={m.description} html /> : <p className="cmp-muted">Pas de briefing.</p>}</Panel>
            <Panel header="Participation">
              {retour && <Message severity={retour.severity} onClose={() => setRetour(null)}>{retour.texte}</Message>}
              {moi && <p className="cmp-small">Ton statut : <strong>{moi.status === 'inscrit' ? 'inscrit' : moi.status === 'reserve' ? (moi.waitlisted ? 'liste d’attente' : 'réserve') : 'désisté'}</strong>{moi.role ? ` · ${moi.role}` : ''}</p>}
              {ouverte ? (<>
                {postes.length > 0 && (
                  <label className="nat-champ"><span className="cmp-small cmp-muted">Poste</span>
                    <Select value={role} onChange={(e) => setRole(e.target.value)} options={[{ label: 'Choisir un poste…', value: '' }, ...postes.map((p) => {
                      const pris = participants.filter((x) => x.role === p.name && x.status === 'inscrit').length;
                      return { label: `${p.name} — ${pris}${p.slots ? ` / ${p.slots}` : ''}`, value: p.name };
                    })]} />
                  </label>
                )}
                <div className="cmp-actions">
                  <Button label={moi?.status === 'inscrit' ? 'Mettre à jour' : 'M’inscrire'} icon="check" loading={occupe} disabled={postes.length > 0 && !role} onClick={() => sInscrire('inscrit')} />
                  {moi?.status !== 'reserve' && <Button variant="outlined" label="En réserve" disabled={occupe || (postes.length > 0 && !role)} onClick={() => sInscrire('reserve')} />}
                  {moi && <Button variant="text" label="Me désinscrire" disabled={occupe} onClick={seDesister} />}
                </div>
              </>) : <p className="cmp-muted">Cette opération n’accepte plus d’inscriptions.</p>}
              <h4 className="nat-h4">Équipage ({participants.filter((p) => p.status === 'inscrit').length})</h4>
              {participants.length === 0 ? <p className="cmp-muted cmp-small">Personne pour l’instant.</p> : Object.entries(parPoste).map(([poste, liste]) => (
                <div key={poste} className="nat-poste">
                  <span className="cmp-small cmp-muted">{poste}</span>
                  <ul>{liste.map((p) => (
                    <li key={p.id} className={p.status !== 'inscrit' ? 'is-reserve' : undefined}><Avatar u={p.User} taille={22} /> {p.User?.username}{p.status !== 'inscrit' && <em> · {p.waitlisted ? 'attente' : p.status}</em>}{p.Ship && <span className="cmp-muted"> · {p.Ship.customName || p.Ship.Ship?.name}</span>}</li>
                  ))}</ul>
                </div>
              ))}
            </Panel>
          </div>
        </>)}
      </Chargement>
    </div>
  );
}

export function Missions({ moiId, cible, setCible }) {
  const etat = useDonnee(() => api('GET', 'events'), []);
  const [onglet, setOnglet] = useState('avenir');
  if (cible) return <DetailMission id={cible} moiId={moiId} onRetour={() => { setCible(null); etat.relire(); }} />;
  const liste = etat.data || [];
  const maintenant = Date.now();
  const avenir = liste.filter((m) => m.status === 'planifie' && new Date(m.dateEnd || m.dateStart).getTime() + 4 * 3600e3 > maintenant);
  const passees = liste.filter((m) => !avenir.includes(m)).reverse();
  const montrees = onglet === 'avenir' ? avenir : passees;
  return (
    <div className="cmp-stack">
      <div className="nat-barre">
        <div className="nat-onglets" role="tablist">
          <button type="button" role="tab" aria-selected={onglet === 'avenir'} className={onglet === 'avenir' ? 'is-actif' : undefined} onClick={() => setOnglet('avenir')}>À venir ({avenir.length})</button>
          <button type="button" role="tab" aria-selected={onglet === 'archives'} className={onglet === 'archives' ? 'is-actif' : undefined} onClick={() => setOnglet('archives')}>Archives ({passees.length})</button>
        </div>
        <span className="nat-barre__actions">
          <Button size="sm" variant="text" icon="undo" label="Actualiser" onClick={etat.relire} />
          <Button size="sm" variant="outlined" icon="plus" label="Créer sur le portail" onClick={() => ouvrirPortail('/create')} />
        </span>
      </div>
      <Chargement etat={etat}>
        {montrees.length === 0
          ? <EmptyState icon="rocket" title={onglet === 'avenir' ? 'Aucune opération à venir' : 'Pas encore d’archives'} text="Les opérations du CES apparaissent ici dès qu’elles sont publiées sur le portail." />
          : onglet === 'avenir'
            ? groupesEcheance(montrees).map(([titre, liste]) => (
              <section key={titre} className="nat-groupe">
                <h3 className="nat-groupe__titre">{titre} <span>{liste.length}</span></h3>
                <div className="nat-grille-missions">{liste.map((m) => <CarteMission key={m.id} m={m} moiId={moiId} onOuvrir={() => setCible(m.id)} />)}</div>
              </section>
            ))
            : <div className="nat-grille-missions">{montrees.slice(0, 60).map((m) => <CarteMission key={m.id} m={m} moiId={moiId} onOuvrir={() => setCible(m.id)} />)}</div>}
      </Chargement>
    </div>
  );
}

// =============================================================================================== Forum
function NouveauSujet({ categorie, onFini }) {
  const [titre, setTitre] = useState('');
  const [texte, setTexte] = useState('');
  const [occupe, setOccupe] = useState(false);
  const [erreur, setErreur] = useState(null);
  const envoyer = async () => {
    setOccupe(true); setErreur(null);
    try { const t = await api('POST', 'forum/topics', { categoryId: categorie.id, title: titre, content: texte }); onFini(t?.id); }
    catch (e) { setErreur(String(e)); }
    finally { setOccupe(false); }
  };
  return (
    <Panel header={`Nouveau sujet — ${categorie.name}`}>
      <div className="cmp-form">
        <InputText value={titre} maxLength={140} placeholder="Titre du sujet" onChange={(e) => setTitre(e.target.value)} />
        <textarea className="hud-saisie" rows={6} maxLength={10000} placeholder="Ton message (Markdown accepté)" value={texte} onChange={(e) => setTexte(e.target.value)} />
        {erreur && <Message severity="danger">{erreur}</Message>}
        <div className="cmp-actions"><Button variant="text" label="Annuler" onClick={() => onFini(null)} /><Button icon="send" label="Publier" loading={occupe} disabled={!titre.trim() || !texte.trim()} onClick={envoyer} /></div>
      </div>
    </Panel>
  );
}

function Sujet({ id, onRetour }) {
  const etat = useDonnee(() => api('GET', `forum/topics/${id}`), [id]);
  const [texte, setTexte] = useState('');
  const [occupe, setOccupe] = useState(false);
  const [erreur, setErreur] = useState(null);
  const t = etat.data?.topic;
  const repondre = async () => {
    setOccupe(true); setErreur(null);
    try { await api('POST', `forum/topics/${id}/posts`, { content: texte }); setTexte(''); await etat.relire(); }
    catch (e) { setErreur(String(e)); }
    finally { setOccupe(false); }
  };
  return (
    <div className="cmp-stack">
      <BarreRetour onRetour={onRetour} libelle={t?.Category?.name || 'Forum'} actions={<Button size="sm" variant="text" icon="link" label="Sur le portail" onClick={() => ouvrirPortail(`/forum/sujet/${id}`)} />} />
      <Chargement etat={etat}>
        {t && (<>
          <h2 className="nat-titre">{t.pinned && <Icon name="pin" size={16} />} {t.locked && <Icon name="lock" size={16} />} {t.title}</h2>
          <ol className="nat-posts">
            {(etat.data.posts || []).map((p) => (
              <li key={p.id} className="nat-post">
                <div className="nat-post__tete"><Avatar u={p.Author} taille={30} /><strong>{p.Author?.username || 'Compte supprimé'}</strong><time className="cmp-muted cmp-small">{dateLongue(p.createdAt)}</time></div>
                {p.deletedAt ? <p className="cmp-muted cmp-small">Message supprimé.</p> : <Markdown texte={p.content} />}
              </li>
            ))}
          </ol>
          {t.locked ? <Message severity="info">Sujet verrouillé.</Message> : (
            <Panel header="Répondre">
              <textarea className="hud-saisie" rows={4} maxLength={10000} placeholder="Ta réponse (Markdown accepté)" value={texte} onChange={(e) => setTexte(e.target.value)} />
              {erreur && <Message severity="danger">{erreur}</Message>}
              <div className="cmp-actions"><Button icon="send" label="Envoyer" loading={occupe} disabled={!texte.trim()} onClick={repondre} /></div>
            </Panel>
          )}
        </>)}
      </Chargement>
    </div>
  );
}

function Categorie({ categorie, onRetour, ouvrirSujet }) {
  const [page, setPage] = useState(0);
  const [creer, setCreer] = useState(false);
  const etat = useDonnee(() => api('GET', `forum/topics?category=${categorie.id}&page=${page}`), [categorie.id, page]);
  if (creer) return <NouveauSujet categorie={categorie} onFini={(id) => { setCreer(false); if (id) ouvrirSujet(id); else etat.relire(); }} />;
  return (
    <div className="cmp-stack">
      <BarreRetour onRetour={onRetour} libelle="Toutes les catégories" actions={<Button size="sm" icon="plus" label="Nouveau sujet" onClick={() => setCreer(true)} />} />
      <h2 className="nat-titre"><i className="nat-pastille" style={{ background: categorie.color || 'var(--ds-primary)' }} /> {categorie.name}</h2>
      <Chargement etat={etat}>
        {(etat.data?.topics || []).length === 0 ? <EmptyState icon="forum" title="Aucun sujet" text="Ouvre le premier sujet de cette catégorie." /> : (
          <ul className="nat-liste">
            {etat.data.topics.map((t) => (
              <li key={t.id}><button type="button" className="nat-ligne" onClick={() => ouvrirSujet(t.id)}>
                <Avatar u={t.Author} taille={30} />
                <span className="nat-ligne__txt"><strong>{t.pinned && <Icon name="pin" size={13} />} {t.title}</strong><small className="cmp-muted">par {t.Author?.username} · {t.replyCount || 0} réponse{t.replyCount > 1 ? 's' : ''}</small></span>
                <span className="cmp-muted cmp-small">{relatif(t.lastPostAt)}</span>
              </button></li>
            ))}
          </ul>
        )}
        {etat.data?.pages > 1 && (
          <div className="cmp-actions">
            <Button size="sm" variant="outlined" label="Précédents" disabled={page === 0} onClick={() => setPage(page - 1)} />
            <span className="cmp-muted cmp-small">Page {page + 1} / {etat.data.pages}</span>
            <Button size="sm" variant="outlined" label="Suivants" disabled={page + 1 >= etat.data.pages} onClick={() => setPage(page + 1)} />
          </div>
        )}
      </Chargement>
    </div>
  );
}

export function Forum({ cible, setCible }) {
  const etat = useDonnee(() => api('GET', 'forum/categories'), []);
  const [cat, setCat] = useState(null);
  const [q, setQ] = useState('');
  const [trouves, setTrouves] = useState(null);
  useEffect(() => {
    const t = q.trim();
    if (t.length < 2) { setTrouves(null); return undefined; }
    const id = setTimeout(() => api('GET', `forum/topics?q=${encodeURIComponent(t)}`).then((v) => setTrouves(v?.topics || [])).catch(() => setTrouves([])), 300);
    return () => clearTimeout(id);
  }, [q]);
  if (cible) return <Sujet id={cible} onRetour={() => setCible(null)} />;
  if (cat) return <Categorie categorie={cat} onRetour={() => { setCat(null); etat.relire(); }} ouvrirSujet={setCible} />;
  return (
    <div className="cmp-stack">
      <div className="nat-barre"><span className="nat-recherche"><Icon name="search" size={16} /><input type="search" placeholder="Chercher un sujet…" value={q} onChange={(e) => setQ(e.target.value)} /></span></div>
      {trouves && (
        <Panel header={`Résultats (${trouves.length})`}>
          {trouves.length === 0 ? <p className="cmp-muted cmp-small">Aucun sujet ne correspond.</p> : (
            <ul className="nat-liste">{trouves.slice(0, 20).map((t) => (
              <li key={t.id}><button type="button" className="nat-ligne" onClick={() => setCible(t.id)}>
                <i className="nat-pastille" style={{ background: t.Category?.color || 'var(--ds-primary)' }} />
                <span className="nat-ligne__txt"><strong>{t.title}</strong><small className="cmp-muted">{t.Category?.name} · par {t.Author?.username}</small></span>
                <span className="cmp-muted cmp-small">{relatif(t.lastPostAt)}</span>
              </button></li>
            ))}</ul>
          )}
        </Panel>
      )}
      <Chargement etat={etat}>
        {(etat.data?.categories || []).length === 0 ? <EmptyState icon="forum" title="Forum vide" text="Aucune catégorie accessible pour l’instant." /> : (
          <ul className="nat-liste">
            {etat.data.categories.map((c) => (
              <li key={c.id}><button type="button" className="nat-ligne nat-ligne--cat" style={{ '--c': c.color || 'var(--ds-primary)' }} onClick={() => setCat(c)}>
                <i className="nat-pastille nat-pastille--grande" style={{ background: c.color || 'var(--ds-primary)' }} />
                <span className="nat-ligne__txt"><strong>{c.name}</strong><small className="cmp-muted">{c.description || `${c.topics} sujet${c.topics > 1 ? 's' : ''} · ${c.posts} message${c.posts > 1 ? 's' : ''}`}</small></span>
                {c.last && <span className="nat-ligne__dernier cmp-small"><span>{c.last.title}</span><span className="cmp-muted">{relatif(c.last.lastPostAt)}</span></span>}
              </button></li>
            ))}
          </ul>
        )}
      </Chargement>
    </div>
  );
}

// =============================================================================================== Annuaire
function Profil({ id, onRetour }) {
  const etat = useDonnee(() => api('GET', `members/${id}`), [id]);
  const u = etat.data;
  const metiers = u?.Jobs || [];
  const certifs = (u?.UserCertifications || []).map((c) => c.Certification?.title).filter(Boolean);
  const vaisseaux = (u?.UserShips || []).map((s) => s.customName || s.Ship?.name).filter(Boolean);
  const ops = (u?.EventParticipants || []).filter((p) => p.status === 'inscrit').length;
  return (
    <div className="cmp-stack">
      <BarreRetour onRetour={onRetour} libelle="Annuaire" actions={<Button size="sm" variant="text" icon="link" label="Profil complet sur le portail" onClick={() => ouvrirPortail(`/profils/member/${id}`)} />} />
      <Chargement etat={etat}>
        {u && (<>
          <section className="nat-hero nat-hero--profil" style={u.banner ? { '--nat-img': `url(${media(u.banner)})` } : undefined}>
            <div className="nat-hero__txt nat-hero__txt--profil">
              <Avatar u={u} taille={72} />
              <div>
                <h2>{u.username}</h2>
                <p>{[u.matricule, u.Rank?.name].filter(Boolean).join(' · ')}</p>
                <span className="nat-hero__tags">{metiers.map((j) => <span key={j.id} className="nat-metier" style={{ '--c': j.color || '#7c9cff' }}>{j.name}</span>)}</span>
              </div>
            </div>
          </section>
          <div className="nat-chiffres">
            <div className="nat-chiffre"><strong>{ops}</strong><span className="cmp-muted cmp-small">Opérations</span></div>
            <div className="nat-chiffre"><strong>{certifs.length}</strong><span className="cmp-muted cmp-small">Certifications</span></div>
            <div className="nat-chiffre"><strong>{vaisseaux.length}</strong><span className="cmp-muted cmp-small">Vaisseaux</span></div>
            <div className="nat-chiffre"><strong>{u.memberSince ? new Date(u.memberSince).getFullYear() : '—'}</strong><span className="cmp-muted cmp-small">Membre depuis</span></div>
          </div>
          <div className="nat-deux">
            <Panel header="Présentation">
              {u.bio ? <p className="nat-bio">{u.bio}</p> : <p className="cmp-muted">Pas de présentation (ou masquée par le membre).</p>}
              <dl className="cmp-dl cmp-gap">
                {u.memberSince && (<><dt>Membre depuis</dt><dd>{dateCourte(u.memberSince)}</dd></>)}
                {u.rsiHandle && (<><dt>Pseudo RSI</dt><dd>{u.rsiHandle}</dd></>)}
                {u.location && (<><dt>Lieu</dt><dd>{u.location}</dd></>)}
                {Array.isArray(u.languages) && u.languages.length > 0 && (<><dt>Langues</dt><dd>{u.languages.join(', ')}</dd></>)}
                <dt>Opérations</dt><dd>{ops}</dd>
              </dl>
            </Panel>
            <Panel header="Qualifications">
              <h4 className="nat-h4">Certifications</h4>
              {certifs.length ? <div className="nat-tags">{certifs.map((c) => <Tag key={c} value={c} />)}</div> : <p className="cmp-muted cmp-small">Aucune visible.</p>}
              <h4 className="nat-h4">Hangar ({vaisseaux.length})</h4>
              {vaisseaux.length ? <div className="nat-tags">{vaisseaux.slice(0, 40).map((v, i) => <Tag key={`${v}-${i}`} value={v} severity="secondary" />)}</div> : <p className="cmp-muted cmp-small">Hangar vide ou masqué.</p>}
            </Panel>
          </div>
        </>)}
      </Chargement>
    </div>
  );
}

export function Annuaire({ cible, setCible }) {
  const etat = useDonnee(() => api('GET', 'members'), []);
  const direct = useDonnee(() => api('GET', 'companion/live').catch(() => null), []);
  const [q, setQ] = useState('');
  const [metier, setMetier] = useState('');
  const [tri, setTri] = useState('nom');
  if (cible) return <Profil id={cible} onRetour={() => setCible(null)} />;
  const enJeu = new Set((direct.data?.playing || []).map((p) => (p.user || p).id));
  const metiers = [...new Map((etat.data || []).flatMap((u) => u.Jobs || []).map((j) => [j.name, j])).values()].sort((a, b) => a.name.localeCompare(b.name));
  const filtre = q.trim().toLowerCase();
  const membres = (etat.data || [])
    .filter((u) => !filtre || `${u.username} ${u.matricule || ''} ${u.Rank?.name || ''} ${(u.Jobs || []).map((j) => j.name).join(' ')}`.toLowerCase().includes(filtre))
    .filter((u) => !metier || (u.Jobs || []).some((j) => j.name === metier))
    .sort((a, b) => (tri === 'jeu' ? Number(enJeu.has(b.id)) - Number(enJeu.has(a.id)) : 0)
      || (tri === 'anciennete' ? String(a.memberSince || a.createdAt || '').localeCompare(String(b.memberSince || b.createdAt || '')) : 0)
      || a.username.localeCompare(b.username));
  return (
    <div className="cmp-stack">
      <div className="nat-barre">
        <span className="nat-recherche"><Icon name="search" size={16} /><input type="search" placeholder="Pseudo, matricule, grade, métier…" value={q} onChange={(e) => setQ(e.target.value)} /></span>
        <Select value={tri} onChange={(e) => setTri(e.target.value)} options={[{ label: 'Trier par nom', value: 'nom' }, { label: 'En jeu d’abord', value: 'jeu' }, { label: 'Plus anciens d’abord', value: 'anciennete' }]} />
        <span className="cmp-muted cmp-small nat-barre__fin">{membres.length} membre{membres.length > 1 ? 's' : ''}{enJeu.size ? ` · ${enJeu.size} en jeu` : ''}</span>
      </div>
      {metiers.length > 0 && (
        <div className="nat-puces" role="radiogroup" aria-label="Métier">
          <button type="button" role="radio" aria-checked={!metier} className={!metier ? 'is-actif' : undefined} onClick={() => setMetier('')}>Tous</button>
          {metiers.map((j) => <button key={j.name} type="button" role="radio" aria-checked={metier === j.name} className={metier === j.name ? 'is-actif' : undefined} style={{ '--c': j.color || '#7c9cff' }} onClick={() => setMetier(metier === j.name ? '' : j.name)}>{j.name}</button>)}
        </div>
      )}
      <Chargement etat={etat}>
        <div className="nat-grille-membres">
          {membres.map((u) => (
            <button key={u.id} type="button" className={`nat-membre${enJeu.has(u.id) ? ' is-en-jeu' : ''}`} onClick={() => setCible(u.id)}>
              <span className="nat-membre__av"><Avatar u={u} taille={48} />{enJeu.has(u.id) && <i title="En jeu" />}</span>
              <span className="nat-membre__txt">
                <strong>{u.username}</strong>
                <small className="cmp-muted">{[u.matricule, u.Rank?.name].filter(Boolean).join(' · ') || '—'}</small>
                <span className="nat-tags">{(u.Jobs || []).slice(0, 3).map((j) => <span key={j.id} className="nat-metier" style={{ '--c': j.color || '#7c9cff' }}>{j.name}</span>)}</span>
              </span>
            </button>
          ))}
        </div>
      </Chargement>
    </div>
  );
}

// =============================================================================================== Mes données
const duree = (s) => { const h = Math.floor((s || 0) / 3600); const m = Math.round(((s || 0) % 3600) / 60); return h ? `${h} h ${String(m).padStart(2, '0')}` : `${m} min`; };
const TYPES_EVT = { 'session.start': 'Début de session', 'session.end': 'Fin de session', 'location.enter': 'Lieu atteint', 'quantum.jump': 'Saut quantique', 'mining.session': 'Session de minage' };

export function MesDonnees({ moiId }) {
  const direct = useDonnee(() => api('GET', 'companion/live'), []);
  const carnet = useDonnee(() => (moiId ? api('GET', `companion/logbook/${moiId}`) : Promise.resolve(null)), [moiId]);
  const envois = useDonnee(() => api('GET', 'companion/events/mine?limit=40'), []);
  const t = carnet.data?.totals;
  return (
    <div className="cmp-stack">
      <div className="nat-chiffres">
        {[['Cette semaine', duree(t?.weekSec)], ['Sur 30 jours', duree(t?.monthSec)], ['Sessions', t?.sessions ?? '—'], ['Sauts quantiques', t?.quantum ?? '—'], ['Missions', t?.missions ?? '—']].map(([l, v]) => (
          <div key={l} className="nat-chiffre"><strong>{v}</strong><span className="cmp-muted cmp-small">{l}</span></div>
        ))}
      </div>
      <div className="nat-deux">
        <Panel header="En jeu maintenant">
          <Chargement etat={direct}>
            {(direct.data?.playing || []).length === 0 ? <p className="cmp-muted">Personne en jeu pour l’instant.</p> : (
              <ul className="nat-liste nat-liste--compacte">{direct.data.playing.map((p) => {
                // `/companion/live` renvoie { user: { id, username, avatar, customAvatar }, state, since, region }.
                const u = p.user || p;
                return (
                  <li key={u.id || u.username} className="nat-ligne nat-ligne--statique"><Avatar u={u} taille={26} />
                    <span className="nat-ligne__txt"><strong>{u.username || 'Membre'}</strong>
                      <small className="cmp-muted">{p.state === 'universe' ? 'Dans le Verse' : p.state === 'menu' ? 'Au menu' : 'En jeu'} · depuis {relatif(p.since).replace('il y a ', '')}</small></span></li>
                );
              })}</ul>
            )}
            {direct.data?.week && <p className="cmp-muted cmp-small cmp-gap">CES sur 7 jours : {duree(direct.data.week.weekSec)} de jeu, {direct.data.week.pilots} pilote{direct.data.week.pilots > 1 ? 's' : ''}.</p>}
          </Chargement>
        </Panel>
        <Panel header="Derniers envois de cet appareil" actions={<Button size="sm" variant="text" icon="link" label="Tout gérer" onClick={() => ouvrirPortail('/compagnon/espace')} />}>
          <Chargement etat={envois}>
            {(envois.data?.events || []).length === 0 ? <p className="cmp-muted">Rien n’a encore été envoyé au portail.</p> : (
              <ul className="nat-envois">{envois.data.events.map((e) => (
                <li key={e.eventId}><span>{TYPES_EVT[e.type] || e.type}{e.data?.name || e.data?.location ? ` — ${e.data.name || e.data.location}` : ''}</span><time className="cmp-muted cmp-small">{dateLongue(e.at)}</time></li>
              ))}</ul>
            )}
          </Chargement>
        </Panel>
      </div>
    </div>
  );
}

// =============================================================================================== DEFCON
const NIVEAUX = [
  { n: 5, nom: 'Paix', c: '#22c55e' }, { n: 4, nom: 'Vigilance', c: '#3b82f6' }, { n: 3, nom: 'Alerte', c: '#ca8a04' },
  { n: 2, nom: 'Alerte rouge', c: '#ea580c' }, { n: 1, nom: 'Guerre', c: '#dc2626' },
];

export function Defcon({ peutGerer }) {
  const etat = useDonnee(() => api('GET', peutGerer ? 'admin/defcon' : 'site/defcon'), [peutGerer]);
  const d = etat.data;
  const [niveau, setNiveau] = useState(3);
  const [message, setMessage] = useState('');
  const [fin, setFin] = useState('');
  const [confirmer, setConfirmer] = useState(null);
  const [occupe, setOccupe] = useState(false);
  const [retour, setRetour] = useState(null);
  const actif = d?.active;
  const courant = NIVEAUX.find((x) => x.n === (actif ? d.level : 5));
  const enregistrer = async (corps) => {
    setOccupe(true); setRetour(null);
    try { await api('PUT', 'admin/defcon', corps); setConfirmer(null); setRetour({ severity: 'success', texte: corps.active ? `DEFCON ${corps.level} déclenché : les membres sont prévenus.` : 'Alerte levée.' }); await etat.relire(); }
    catch (e) { setRetour({ severity: 'danger', texte: String(e) }); }
    finally { setOccupe(false); }
  };
  const declencher = () => {
    const corps = { active: true, level: niveau, message, until: fin ? new Date(fin).toISOString() : null };
    if (niveau <= 3) setConfirmer(corps); else enregistrer(corps);
  };
  const historique = d?.history || [];
  return (
    <div className="cmp-stack">
      <Chargement etat={etat}>
        <section className="nat-defcon" style={{ '--dc': courant?.c }}>
          <div className="nat-defcon__niveau"><span>DEFCON</span><strong>{actif ? d.level : 5}</strong></div>
          <div className="nat-defcon__txt">
            <h2>{actif ? d.name || courant?.nom : 'Aucune alerte en cours'}</h2>
            {actif && d.message && <p>{d.message}</p>}
            {actif && <p className="cmp-muted cmp-small">Par {d.setBy || '—'} · {dateLongue(d.setAt)}{d.until ? ` · jusqu’au ${dateLongue(d.until)}` : ''}</p>}
          </div>
          <div className="nat-defcon__jauge" aria-hidden="true">{NIVEAUX.map((x) => <i key={x.n} className={(actif ? d.level : 5) <= x.n ? 'is-on' : undefined} style={{ '--c': x.c }} />)}</div>
        </section>
        {retour && <Message severity={retour.severity} onClose={() => setRetour(null)}>{retour.texte}</Message>}
        {peutGerer && (
          <Panel header="Gestion de l’alerte">
            <div className="nat-niveaux" role="radiogroup" aria-label="Niveau">
              {NIVEAUX.slice().reverse().map((x) => (
                <button key={x.n} type="button" role="radio" aria-checked={niveau === x.n} className={niveau === x.n ? 'nat-niveau is-actif' : 'nat-niveau'} style={{ '--c': x.c }} onClick={() => setNiveau(x.n)}>
                  <strong>{x.n}</strong><span>{x.nom}</span>
                </button>
              ))}
            </div>
            <textarea className="hud-saisie cmp-gap" rows={3} maxLength={600} placeholder="Consigne aux membres (600 caractères)" value={message} onChange={(e) => setMessage(e.target.value)} />
            <label className="nat-champ cmp-gap"><span className="cmp-small cmp-muted">Fin automatique (facultatif)</span>
              <input className="ds-input" type="datetime-local" value={fin} onChange={(e) => setFin(e.target.value)} /></label>
            {confirmer ? (
              <div className="hud-confirmer cmp-gap">
                <p><strong>Déclencher DEFCON {confirmer.level} ?</strong> Tous les membres reçoivent une notification et la fenêtre d’alerte s’ouvre chez eux.</p>
                <div className="cmp-actions"><Button variant="outlined" label="Annuler" onClick={() => setConfirmer(null)} /><Button severity="danger" label={`Oui, DEFCON ${confirmer.level}`} loading={occupe} onClick={() => enregistrer(confirmer)} /></div>
              </div>
            ) : (
              <div className="cmp-actions">
                <Button icon="alert" label={`Passer en DEFCON ${niveau}`} loading={occupe} onClick={declencher} />
                {actif && <Button variant="outlined" label="Lever l’alerte" disabled={occupe} onClick={() => enregistrer({ active: false })} />}
              </div>
            )}
          </Panel>
        )}
        <Panel header="Bulletins précédents">
          {historique.length === 0 ? <p className="cmp-muted">Aucun bulletin.</p> : (
            <ul className="nat-envois">{historique.map((h, i) => (
              <li key={`${h.setAt}-${i}`}><span><i className="nat-pastille" style={{ background: NIVEAUX.find((x) => x.n === h.level)?.c }} /> DEFCON {h.level} — {h.name}{h.message ? ` : ${h.message}` : ''}</span><time className="cmp-muted cmp-small">{dateCourte(h.setAt)} · {h.lifted ? 'levée' : 'remplacée'}{h.endedBy ? ` par ${h.endedBy}` : ''}</time></li>
            ))}</ul>
          )}
        </Panel>
      </Chargement>
    </div>
  );
}

// =============================================================================================== Ce soir
const parisJour = (iso) => new Date(iso).toLocaleString('fr-FR', { weekday: 'long', day: 'numeric', month: 'long', hour: '2-digit', minute: '2-digit' });

/** Soirées proposées par les membres (portail /ce-soir, commande Discord /soiree) : je viens / peut-être, proposer. */
export function CeSoir({ moi }) {
  const etat = useDonnee(() => api('GET', 'soirees'), []);
  const [form, setForm] = useState(false);
  const [activite, setActivite] = useState('');
  const [heure, setHeure] = useState('21:00');
  const [places, setPlaces] = useState('');
  const [note, setNote] = useState('');
  const [occupe, setOccupe] = useState(null);
  const [retour, setRetour] = useState(null);
  const agir = async (cle, fn, ok) => {
    setOccupe(cle); setRetour(null);
    try { await fn(); setRetour(ok ? { severity: 'success', texte: ok } : null); await etat.relire(); return true; }
    catch (e) { setRetour({ severity: 'danger', texte: String(e) }); return false; }
    finally { setOccupe(null); }
  };
  const proposer = async () => {
    const [h, m] = heure.split(':').map(Number);
    const d = new Date(); d.setHours(h, m || 0, 0, 0); if (d < new Date()) d.setDate(d.getDate() + 1);
    const ok = await agir('creer', () => api('POST', 'soirees', { activity: activite, startsAt: d.toISOString(), slots: places ? Number(places) : null, note }), 'Soirée proposée : elle est annoncée sur Discord.');
    if (ok) { setForm(false); setActivite(''); setNote(''); }
  };
  const liste = etat.data?.soirees || [];
  return (
    <div className="cmp-stack">
      <div className="nat-barre">
        <span className="cmp-muted cmp-small">Envie de jouer ce soir ? Propose une activité ou rejoins celle d’un autre membre.</span>
        <span className="nat-barre__actions"><Button size="sm" variant="text" icon="undo" label="Actualiser" onClick={etat.relire} /><Button size="sm" icon="plus" label="Proposer une soirée" onClick={() => setForm(!form)} /></span>
      </div>
      {retour && <Message severity={retour.severity} onClose={() => setRetour(null)}>{retour.texte}</Message>}
      {form && (
        <Panel header="Proposer une soirée">
          <div className="cmp-form">
            <InputText value={activite} maxLength={80} placeholder="Activité (ex. minage à Aaron Halo, chasse aux primes…)" onChange={(e) => setActivite(e.target.value)} />
            <div className="cmp-deux">
              <label className="nat-champ"><span className="cmp-small cmp-muted">Heure</span><input className="ds-input" type="time" value={heure} onChange={(e) => setHeure(e.target.value)} /></label>
              <label className="nat-champ"><span className="cmp-small cmp-muted">Places (facultatif)</span><input className="ds-input" type="number" min="1" max="50" value={places} onChange={(e) => setPlaces(e.target.value)} /></label>
            </div>
            <InputText value={note} maxLength={200} placeholder="Note (facultatif)" onChange={(e) => setNote(e.target.value)} />
            <div className="cmp-actions"><Button variant="text" label="Annuler" onClick={() => setForm(false)} /><Button icon="send" label="Proposer" loading={occupe === 'creer'} disabled={!activite.trim()} onClick={proposer} /></div>
          </div>
        </Panel>
      )}
      <Chargement etat={etat}>
        {liste.length === 0 ? <EmptyState icon="calendar" title="Rien de prévu ce soir" text="Sois le premier à proposer une activité." /> : (
          <div className="nat-grille-soirees">
            {liste.map((s) => {
              const moiDedans = (s.participants || []).find((p) => p.name === moi || p.username === moi);
              const annulee = s.status === 'cancelled';
              return (
                <article key={s.id} className={`nat-soiree${annulee ? ' is-annulee' : ''}`}>
                  <div className="nat-soiree__tete">
                    <strong>{s.activity}</strong>
                    <span className="nat-soiree__heure">{new Date(s.startsAt).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' })}</span>
                  </div>
                  <span className="cmp-muted cmp-small">{parisJour(s.startsAt)}{s.organizerName || s.createdByName ? ` · par ${s.organizerName || s.createdByName}` : ''}</span>
                  {s.note && <p className="cmp-small">{s.note}</p>}
                  <div className="nat-soiree__jauge"><span><b>{s.yes}</b>{s.slots ? ` / ${s.slots}` : ''} viennent</span>{s.maybe > 0 && <span className="cmp-muted">· {s.maybe} peut-être</span>}</div>
                  {(s.participants || []).length > 0 && <div className="nat-tags">{s.participants.map((p) => <Tag key={p.name} value={p.name} severity={p.status === 'yes' ? 'success' : 'secondary'} />)}</div>}
                  {!annulee && (
                    <div className="cmp-actions">
                      <Button size="sm" label="Je viens" icon="check" variant={moiDedans?.status === 'yes' ? undefined : 'outlined'} loading={occupe === `y-${s.id}`} onClick={() => agir(`y-${s.id}`, () => api('POST', `soirees/${s.id}/status`, { status: moiDedans?.status === 'yes' ? null : 'yes' }))} />
                      <Button size="sm" label="Peut-être" variant={moiDedans?.status === 'maybe' ? undefined : 'text'} loading={occupe === `m-${s.id}`} onClick={() => agir(`m-${s.id}`, () => api('POST', `soirees/${s.id}/status`, { status: moiDedans?.status === 'maybe' ? null : 'maybe' }))} />
                    </div>
                  )}
                  {annulee && <Tag value="Annulée" severity="danger" />}
                </article>
              );
            })}
          </div>
        )}
      </Chargement>
    </div>
  );
}

// =============================================================================================== Contrats du jeu
const UEC = (n) => `${Math.round(n || 0).toLocaleString('fr-FR')}`;
const sansBalises = (t) => String(t || '').replace(/<\/?EM\d>/g, '');

function FicheContrat({ uuid, onRetour }) {
  const etat = useDonnee(() => api('GET', `contracts/${uuid}`), [uuid]);
  const c = etat.data;
  const d = c?.detail || {};
  return (
    <div className="cmp-stack">
      <BarreRetour onRetour={onRetour} libelle="Tous les contrats" actions={<Button size="sm" variant="text" icon="link" label="Sur le portail" onClick={() => ouvrirPortail('/contrats')} />} />
      <Chargement etat={etat}>
        {c && (<>
          <section className="nat-contrat-tete">
            <span className="nat-hero__tags">{c.typeFr && <Tag value={c.typeFr} />}{c.illegal && <Tag value="Illégal" severity="danger" />}{c.once && <Tag value="Unique" severity="warn" />}{c.shareable && <Tag value="Partageable" severity="secondary" />}</span>
            <h2 className="nat-titre">{c.titleFr || c.title}</h2>
            <p className="cmp-muted cmp-small">{[c.giver, c.faction, (c.systems || []).join(', ')].filter(Boolean).join(' · ')}</p>
            <div className="nat-chiffres">
              <div className="nat-chiffre"><strong>{c.rewardMax ? `${UEC(c.rewardMin)}${c.rewardMax !== c.rewardMin ? ` – ${UEC(c.rewardMax)}` : ''}` : '—'}</strong><span className="cmp-muted cmp-small">Récompense ({c.currency})</span></div>
              {c.rep != null && <div className="nat-chiffre"><strong>{c.rep}</strong><span className="cmp-muted cmp-small">Réputation</span></div>}
              {c.maxPlayers && <div className="nat-chiffre"><strong>{c.maxPlayers}</strong><span className="cmp-muted cmp-small">Joueurs max.</span></div>}
              {d.timeLimit && <div className="nat-chiffre"><strong>{d.timeLimit} min</strong><span className="cmp-muted cmp-small">Temps imparti</span></div>}
            </div>
          </section>
          <div className="nat-deux">
            <Panel header="Description">{d.descriptionFr || d.description ? <p className="nat-bio">{sansBalises(d.descriptionFr || d.description)}</p> : <p className="cmp-muted">Pas de description.</p>}</Panel>
            <Panel header="Conditions et récompenses">
              {c.minStanding && <p className="cmp-small">Réputation requise : <strong>{c.minStanding.nameFr || c.minStanding.name}</strong></p>}
              {d.rewardItems?.length > 0 && (<><h4 className="nat-h4">Objets reçus</h4><ul className="nat-envois">{d.rewardItems.map((i, n) => <li key={n}><span>{i.nameFr || i.name}</span><span className="cmp-muted">{i.amount ? `× ${i.amount}` : ''}</span></li>)}</ul></>)}
              {d.hauling?.length > 0 && (<><h4 className="nat-h4">Transport</h4><ul className="nat-envois">{d.hauling.map((h, n) => <li key={n}><span>{h.nameFr || h.name}</span><span className="cmp-muted">{h.minScu ?? h.min ?? ''}{(h.maxScu ?? h.max) ? ` – ${h.maxScu ?? h.max}` : ''} SCU</span></li>)}</ul></>)}
              {d.blueprints?.length > 0 && (<><h4 className="nat-h4">Plans possibles</h4><div className="nat-tags">{d.blueprints.flatMap((b) => b.items).map((i, n) => <Tag key={n} value={i.nameFr || i.name} severity="secondary" />)}</div></>)}
              {!c.minStanding && !d.rewardItems?.length && !d.hauling?.length && !d.blueprints?.length && <p className="cmp-muted cmp-small">Rien de particulier.</p>}
            </Panel>
          </div>
        </>)}
      </Chargement>
    </div>
  );
}

/** Contrats du jeu (synchronisés par le portail depuis le wiki Star Citizen, titres en français). */
export function Contrats({ cible, setCible }) {
  const etat = useDonnee(() => api('GET', 'contracts'), []);
  const [q, setQ] = useState('');
  const [type, setType] = useState('');
  if (cible) return <FicheContrat uuid={cible} onRetour={() => setCible(null)} />;
  const tous = etat.data?.contracts || [];
  const types = [...new Set(tous.map((c) => c.typeFr).filter(Boolean))].sort();
  const f = q.trim().toLowerCase();
  const liste = tous.filter((c) => (!type || c.typeFr === type) && (!f || `${c.titleFr || ''} ${c.title} ${c.giver || ''} ${c.faction || ''}`.toLowerCase().includes(f)))
    .sort((a, b) => (b.rewardMax || 0) - (a.rewardMax || 0));
  return (
    <div className="cmp-stack">
      <div className="nat-barre">
        <span className="nat-recherche"><Icon name="search" size={16} /><input type="search" placeholder="Titre, donneur d’ordre, faction…" value={q} onChange={(e) => setQ(e.target.value)} /></span>
        <Select value={type} onChange={(e) => setType(e.target.value)} options={[{ label: 'Tous les types', value: '' }, ...types.map((t) => ({ label: t, value: t }))]} />
        <span className="cmp-muted cmp-small nat-barre__fin">{liste.length} contrat{liste.length > 1 ? 's' : ''}</span>
      </div>
      <Chargement etat={etat}>
        {tous.length === 0 ? <EmptyState icon="file" title="Aucun contrat" text="La liste des contrats n’a pas encore été synchronisée par le portail." /> : (
          <div className="ctr-grille">
            {liste.slice(0, 300).map((c) => (
              <button key={c.uuid} type="button" className={`ctr${c.illegal ? ' is-illegal' : ''}`} onClick={() => setCible(c.uuid)}>
                <span className="ctr__haut">
                  {c.typeFr && <Tag value={c.typeFr} />}
                  {c.illegal && <Tag value="Illégal" severity="danger" />}
                  {c.once && <Tag value="Unique" severity="warn" />}
                  {c.chain && <Tag value="Chaîne" severity="secondary" />}
                </span>
                <strong className="ctr__titre">{c.titleFr || c.title}</strong>
                <span className="ctr__donneur cmp-small cmp-muted">{[c.giver, c.faction].filter(Boolean).join(' · ') || '—'}</span>
                <span className="ctr__pied">
                  <span className="ctr__lieu cmp-small"><Icon name="pin" size={12} /> {(c.systems || []).join(', ') || 'Tous systèmes'}</span>
                  <span className="ctr__prime">{c.rewardMax ? `${UEC(c.rewardMax)} ${c.currency === 'UEC' ? 'aUEC' : c.currency || ''}` : '—'}</span>
                </span>
              </button>
            ))}
          </div>
        )}
      </Chargement>
      {etat.data?.sync?.at && <p className="cmp-muted cmp-small">Données du wiki Star Citizen (starcitizen.tools), synchronisées le {dateCourte(etat.data.sync.at)}.</p>}
    </div>
  );
}

// =============================================================================================== Notifications
const NOTIF_TYPE = {
  sos: { icon: 'alert', c: '#ef4444', l: 'SOS' }, defcon: { icon: 'shield', c: '#f59e0b', l: 'DEFCON' },
  forum_reply: { icon: 'forum', c: '#a78bfa', l: 'Forum' }, chat_mention: { icon: 'chat', c: '#a78bfa', l: 'Mention' },
  event_join: { icon: 'rocket', c: '#38bdf8', l: 'Opération' }, event_reminder: { icon: 'time', c: '#38bdf8', l: 'Rappel' },
  task_assigned: { icon: 'kanban', c: '#22c55e', l: 'Tâche' }, board_invite: { icon: 'kanban', c: '#22c55e', l: 'Planning' },
  exam_result: { icon: 'book', c: '#facc15', l: 'Académie' },
};
const typeNotif = (t) => NOTIF_TYPE[t] || (String(t || '').startsWith('event') ? NOTIF_TYPE.event_join : { icon: 'bell', c: '#7c9cff', l: 'Portail' });
const jourNotif = (iso) => {
  const d = new Date(iso); const a = new Date(); a.setHours(0, 0, 0, 0);
  const j = Math.round((a - new Date(d.getFullYear(), d.getMonth(), d.getDate())) / 86400000);
  return j <= 0 ? 'Aujourd’hui' : j === 1 ? 'Hier' : j < 7 ? 'Cette semaine' : 'Plus tôt';
};

/** Notifications du portail dans un écran de l'application (demande du CES, 24/11), plus de tiroir façon site. */
export function Notifications({ onOuvrir, onChange }) {
  const [items, setItems] = useState(null);
  const [filtre, setFiltre] = useState('toutes');
  const [erreur, setErreur] = useState(null);
  const act = (action, id) => invoke('notifications_action', { action, id });
  const lire = useCallback(() => act('liste').then((d) => { setErreur(null); const l = Array.isArray(d?.items) ? d.items : []; setItems(l); onChange?.(l.filter((n) => !n.read).length); })
    .catch((e) => { setErreur(String(e)); setItems([]); }), [onChange]);
  useEffect(() => { lire(); const t = setInterval(lire, 60000); return () => clearInterval(t); }, [lire]);
  const maj = (f) => setItems((l) => { const n = f(l || []); onChange?.(n.filter((x) => !x.read).length); return n; });
  const ouvrir = (n) => {
    if (!n.read) { maj((l) => l.map((x) => (x.id === n.id ? { ...x, read: true } : x))); act('lue', n.id).catch(() => {}); }
    onOuvrir(n);
  };
  const nonLues = (items || []).filter((n) => !n.read).length;
  const montrees = (items || []).filter((n) => filtre === 'toutes' || (filtre === 'nonlues' ? !n.read : typeNotif(n.type).l === filtre));
  const categories = [...new Set((items || []).map((n) => typeNotif(n.type).l))];
  const groupes = montrees.reduce((a, n) => { const k = jourNotif(n.createdAt); (a[k] ||= []).push(n); return a; }, {});
  return (
    <div className="cmp-stack">
      <div className="nat-barre">
        <div className="nat-puces" role="radiogroup" aria-label="Filtrer">
          {[['toutes', 'Toutes'], ['nonlues', `Non lues${nonLues ? ` · ${nonLues}` : ''}`], ...categories.map((c) => [c, c])].map(([v, l]) => (
            <button key={v} type="button" role="radio" aria-checked={filtre === v} className={filtre === v ? 'is-actif' : undefined} onClick={() => setFiltre(v)}>{l}</button>
          ))}
        </div>
        <span className="nat-barre__actions">
          {nonLues > 0 && <Button size="sm" variant="text" icon="check" label="Tout marquer comme lu" onClick={() => { maj((l) => l.map((x) => ({ ...x, read: true }))); act('tout_lu').catch(() => {}); }} />}
          {(items || []).some((n) => n.read) && <Button size="sm" variant="text" icon="trash" label="Effacer les lues" onClick={() => { maj((l) => l.filter((x) => !x.read)); act('effacer_lues').catch(() => {}); }} />}
        </span>
      </div>
      {erreur && <Message severity="warn" title="Notifications indisponibles">{erreur}</Message>}
      {!items ? <p className="cmp-muted">Chargement…</p> : montrees.length === 0
        ? <EmptyState icon="bell" title={filtre === 'nonlues' ? 'Tout est lu' : 'Aucune notification'} text="Opérations, SOS, DEFCON, réponses du forum et invitations arrivent ici (et dans Windows)." />
        : Object.entries(groupes).map(([jour, liste]) => (
          <section key={jour} className="nat-groupe">
            <h3 className="nat-groupe__titre">{jour} <span>{liste.length}</span></h3>
            <ul className="ntn-liste">
              {liste.map((n) => {
                const t = typeNotif(n.type);
                return (
                  <li key={n.id} className={n.read ? 'ntn' : 'ntn is-nonlue'} style={{ '--c': t.c }}>
                    <button type="button" className="ntn__corps" onClick={() => ouvrir(n)}>
                      <span className="ntn__ico"><Icon name={t.icon} size={16} /></span>
                      <span className="ntn__txt"><strong>{n.title}</strong>{n.body && <small>{n.body}</small>}</span>
                      <span className="ntn__meta"><em>{t.l}</em><time>{relatif(n.createdAt)}</time></span>
                    </button>
                    <button type="button" className="ntn__suppr" aria-label="Supprimer cette notification" title="Supprimer"
                      onClick={() => { maj((l) => l.filter((x) => x.id !== n.id)); act('supprimer', n.id).catch(() => {}); }}><Icon name="close" size={14} /></button>
                  </li>
                );
              })}
            </ul>
          </section>
        ))}
    </div>
  );
}
