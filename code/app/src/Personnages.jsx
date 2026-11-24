// Personnages (demande du CES, 21/11) : les apparences créées dans Star Citizen (.chf) sauvegardées sur le portail,
// avec une photo du visage, et remises dans le jeu sur n'importe quel PC.
import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Icon, Button, Message, Panel, EmptyState } from '@aurore/components';

const jour = (iso) => (iso ? new Date(iso).toLocaleDateString('fr-FR', { day: 'numeric', month: 'short', year: 'numeric' }) : '');

export function Personnages({ jeuLance }) {
  const [locaux, setLocaux] = useState(null);
  const [enLigne, setEnLigne] = useState(null);
  const [erreurLocale, setErreurLocale] = useState(null);
  const [erreurPortail, setErreurPortail] = useState(null);
  const [occupe, setOccupe] = useState(null);
  const [retour, setRetour] = useState(null);
  const [photos, setPhotos] = useState({}); // fichier local → photo choisie avant l'envoi
  const relire = useCallback(() => {
    invoke('personnages_locaux').then((l) => { setLocaux(l); setErreurLocale(null); }).catch((e) => setErreurLocale(String(e)));
    invoke('personnages_portail').then((v) => { setEnLigne(v?.characters || []); setErreurPortail(null); }).catch((e) => { setEnLigne(null); setErreurPortail(String(e)); });
  }, []);
  useEffect(() => { relire(); }, [relire]);
  const agir = async (cle, fn, ok) => {
    setOccupe(cle); setRetour(null);
    try { const r = await fn(); setRetour({ severity: 'success', texte: typeof r === 'string' ? r : ok }); relire(); }
    catch (e) { setRetour({ severity: 'danger', texte: String(e) }); }
    finally { setOccupe(null); }
  };
  const choisirPhoto = async (fichier) => { const p = await invoke('personnage_choisir_photo'); if (p) setPhotos((x) => ({ ...x, [fichier]: p })); };
  const envoyer = (l) => agir(`env-${l.fichier}`, () => invoke('personnage_envoyer', { fichier: l.fichier, nom: l.nom, photo: photos[l.fichier] || null }), `« ${l.nom} » est sauvegardé sur le portail.`);
  const changerPhoto = async (c) => { const p = await invoke('personnage_choisir_photo'); if (p) agir(`ph-${c.id}`, () => invoke('personnage_photo', { id: c.id, photo: p }), 'Photo enregistrée.'); };
  const recuperer = (c) => agir(`rec-${c.id}`, () => invoke('personnage_recuperer', { id: c.id, nom: c.fileName || c.name }), '');
  const dejaEnLigne = (l) => (enLigne || []).some((c) => (c.fileName || '').toLowerCase() === l.fichier.toLowerCase() || (c.name || '').toLowerCase() === l.nom.toLowerCase());
  const indispo = erreurPortail && /404|introuvable|not.?found/i.test(erreurPortail);
  return (
    <div className="cmp-stack">
      {retour && <Message severity={retour.severity} onClose={() => setRetour(null)}>{retour.texte}</Message>}
      <div className="nat-deux">
        <Panel header="Sur ce PC" actions={<Button size="sm" variant="text" icon="undo" label="Actualiser" onClick={relire} />}>
          {erreurLocale ? <Message severity="warn">{erreurLocale}</Message>
            : !locaux ? <p className="cmp-muted">Lecture des personnages…</p>
            : locaux.length === 0 ? <EmptyState icon="user" title="Aucun personnage" text="Crée et enregistre une apparence dans le jeu (création de personnage) : elle apparaîtra ici." />
            : (
              <ul className="nat-liste">
                {locaux.map((l) => (
                  <li key={l.fichier} className="nat-ligne nat-ligne--statique per-ligne">
                    <span className="per-vignette">{photos[l.fichier] ? <Icon name="check" size={18} /> : <Icon name="user" size={18} />}</span>
                    <span className="nat-ligne__txt"><strong>{l.nom}</strong><small className="cmp-muted">{jour(l.modifie)}{dejaEnLigne(l) ? ' · déjà sur le portail' : ''}{photos[l.fichier] ? ' · photo prête' : ''}</small></span>
                    <span className="per-actions">
                      <Button size="sm" variant="text" icon="upload" label={photos[l.fichier] ? 'Autre photo' : 'Photo'} onClick={() => choisirPhoto(l.fichier)} />
                      <Button size="sm" icon="upload" label={dejaEnLigne(l) ? 'Renvoyer' : 'Sauvegarder'} loading={occupe === `env-${l.fichier}`} disabled={!!indispo} onClick={() => envoyer(l)} />
                    </span>
                  </li>
                ))}
              </ul>
            )}
          <p className="cmp-muted cmp-small cmp-gap">Les apparences viennent du dossier du jeu (user\client\0\customcharacters). L’application les lit seulement.</p>
        </Panel>
        <Panel header="Sauvegardés sur le portail">
          {indispo ? <Message severity="info">La sauvegarde en ligne des personnages arrive avec la prochaine mise à jour du portail.</Message>
            : erreurPortail ? <Message severity="warn">{erreurPortail}</Message>
            : !enLigne ? <p className="cmp-muted">Chargement…</p>
            : enLigne.length === 0 ? <EmptyState icon="upload" title="Rien de sauvegardé" text="Sauvegarde un personnage de ce PC : tu pourras le remettre dans le jeu sur n’importe quel PC." />
            : (
              <div className="per-grille">
                {enLigne.map((c) => (
                  <figure key={c.id} className="per-carte">
                    {c.photoUrl ? <img src={`http://cesmedia.localhost/__api${c.photoUrl.startsWith('/') ? '' : '/'}${c.photoUrl}`} alt={`Visage de ${c.name}`} loading="lazy" /> : <span className="per-carte__vide"><Icon name="user" size={34} /></span>}
                    <figcaption>
                      <strong>{c.name}</strong>
                      <small className="cmp-muted">{jour(c.updatedAt)}</small>
                      <span className="per-actions">
                        <Button size="sm" icon="download" label="Mettre dans le jeu" loading={occupe === `rec-${c.id}`} disabled={jeuLance} onClick={() => recuperer(c)} />
                        <Button size="sm" variant="text" icon="upload" label={c.photoUrl ? 'Changer la photo' : 'Ajouter une photo'} loading={occupe === `ph-${c.id}`} onClick={() => changerPhoto(c)} />
                      </span>
                    </figcaption>
                  </figure>
                ))}
              </div>
            )}
          {jeuLance && <p className="cmp-muted cmp-small cmp-gap">Ferme Star Citizen pour mettre un personnage dans le jeu.</p>}
        </Panel>
      </div>
    </div>
  );
}
