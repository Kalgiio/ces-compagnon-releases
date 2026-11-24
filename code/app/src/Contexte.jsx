// Clic droit maison et raccourcis du navigateur coupés (demande du CES, 24/11) : plus jamais le menu de WebView2
// (« Copier le lien à mettre en évidence », « Imprimer », « Outils supplémentaires »…), même avec Maj + clic droit ;
// F5 / Ctrl+R (rechargement), Ctrl+P (impression), Ctrl+F, Ctrl+U et les outils de développement sont désactivés.
import React, { useEffect, useState } from 'react';
import { Icon } from '@aurore/components';

const editable = (t) => t?.closest?.('input:not([type=checkbox]):not([type=radio]):not([type=range]), textarea, [contenteditable="true"]');

/** Raccourcis du navigateur intégré qu'une application de bureau ne doit pas laisser passer. */
function bloquerTouches(e) {
  const k = e.key?.toLowerCase();
  const ctrl = e.ctrlKey || e.metaKey;
  if (k === 'f5' || k === 'f12' || k === 'f7' || (ctrl && ['r', 'p', 'f', 'u', 'g', 'j', 's', 'o', 'h'].includes(k)) || (ctrl && e.shiftKey && ['i', 'c', 'r'].includes(k))) {
    e.preventDefault();
    e.stopPropagation();
  }
}

export function MenuContextuel({ actions }) {
  const [menu, setMenu] = useState(null); // { x, y, cible, selection }
  useEffect(() => {
    const ouvrir = (e) => {
      e.preventDefault(); // toujours, Maj comprise : le menu natif ne s'ouvre jamais
      const selection = String(window.getSelection?.() || '');
      const cible = editable(e.target);
      const largeur = 220; const hauteur = 260;
      setMenu({ x: Math.min(e.clientX, window.innerWidth - largeur - 6), y: Math.min(e.clientY, window.innerHeight - hauteur - 6), cible, selection });
    };
    const fermer = () => setMenu(null);
    const echap = (e) => { if (e.key === 'Escape') fermer(); };
    document.addEventListener('contextmenu', ouvrir);
    document.addEventListener('keydown', bloquerTouches, true);
    document.addEventListener('keydown', echap);
    window.addEventListener('blur', fermer);
    window.addEventListener('resize', fermer);
    document.addEventListener('scroll', fermer, true);
    return () => {
      document.removeEventListener('contextmenu', ouvrir);
      document.removeEventListener('keydown', bloquerTouches, true);
      document.removeEventListener('keydown', echap);
      window.removeEventListener('blur', fermer);
      window.removeEventListener('resize', fermer);
      document.removeEventListener('scroll', fermer, true);
    };
  }, []);
  if (!menu) return null;
  const faire = (f) => () => { setMenu(null); f(); };
  const { cible, selection } = menu;
  const copier = () => navigator.clipboard?.writeText(selection || (cible ? cible.value?.slice(cible.selectionStart, cible.selectionEnd) : '')).catch(() => {});
  const couper = () => { cible?.focus(); copier(); document.execCommand('delete'); };
  const coller = async () => { cible?.focus(); try { const t = await navigator.clipboard.readText(); document.execCommand('insertText', false, t); } catch { /* presse-papiers refusé */ } };
  const toutSelectionner = () => { if (cible) { cible.focus(); cible.select?.(); } };
  const entrees = [
    cible && { label: 'Couper', icon: 'edit', f: couper, off: !selection && cible.selectionStart === cible.selectionEnd },
    (cible || selection) && { label: 'Copier', icon: 'copy', f: copier, off: !selection && !(cible && cible.selectionStart !== cible.selectionEnd) },
    cible && { label: 'Coller', icon: 'file', f: coller },
    cible && { label: 'Tout sélectionner', icon: 'check', f: toutSelectionner },
    (cible || selection) && 'sep',
    ...actions,
  ].filter(Boolean);
  return (
    <>
      <div className="ctx-voile" onMouseDown={() => setMenu(null)} onContextMenu={(e) => { e.preventDefault(); setMenu(null); }} />
      <div className="ctx-menu" role="menu" style={{ left: menu.x, top: menu.y }} onMouseDown={(e) => e.preventDefault()}>
        {entrees.map((x, i) => (x === 'sep' ? <i key={`s${i}`} className="ctx-sep" /> : (
          <button key={x.label} type="button" role="menuitem" className="ctx-item" disabled={x.off} onClick={faire(x.f)}>
            {x.icone || <Icon name={x.icon} size={15} />}<span>{x.label}</span>{x.raccourci && <kbd>{x.raccourci}</kbd>}
          </button>
        )))}
      </div>
    </>
  );
}
