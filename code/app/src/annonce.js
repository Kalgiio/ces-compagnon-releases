// Carte CES sur le bureau (d'après la maquette de le CES, 22/11) : avatar ou initiales, nom, « a lancé Star Citizen ».
// Le contenu arrive dans l'adresse (#…) ; aucun appel au cœur, aucune donnée stockée. Texte inséré en textContent.
import '@fontsource-variable/dm-sans';
import './annonce.css';

let d = {};
try { d = JSON.parse(decodeURIComponent(location.hash.slice(1))); } catch { /* adresse vide */ }

const carte = document.getElementById('carte');
carte.className = `carte carte--${d.ton === 'sos' ? 'sos' : 'lancement'}`;

const pastille = document.createElement('div');
pastille.className = 'carte__pastille';
if (d.image) {
  const img = document.createElement('img');
  img.src = d.image;
  img.alt = '';
  pastille.appendChild(img);
} else {
  const nom = String(d.nom || d.titre || '?').trim();
  pastille.textContent = nom.slice(0, 2).toUpperCase();
}

const txt = document.createElement('div');
txt.className = 'carte__txt';
const titre = document.createElement('strong');
titre.textContent = String(d.titre || 'CES Compagnon');
const sous = document.createElement('span');
sous.textContent = String(d.texte || '');
txt.append(titre, sous);

const marque = document.createElement('span');
marque.className = 'carte__marque';
marque.textContent = d.ton === 'sos' ? 'SOS' : 'CES';

carte.append(pastille, txt, marque);
requestAnimationFrame(() => carte.classList.add('is-visible'));
setTimeout(() => carte.classList.remove('is-visible'), 6400);
