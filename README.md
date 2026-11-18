<p align="center">
  <img src="https://app.ces-sc.fr/favicon.svg" width="88" alt="CES Compagnon">
</p>

<h1 align="center">CES Compagnon</h1>

<p align="center">
  L'application Windows du <strong>Corps Expéditionnaire Spatial</strong>, organisation francophone sur Star Citizen.<br>
  Votre jeu, le portail du CES et vos outils de vol, réunis dans une seule fenêtre.
</p>

<p align="center">
  <a href="https://github.com/Kalgiio/ces-compagnon-releases/releases/tag/canary"><img src="https://img.shields.io/badge/télécharger-Windows%2010%20%2F%2011-5b8def?style=for-the-badge" alt="Télécharger"></a>
  <a href="https://status.ces-sc.fr"><img src="https://img.shields.io/badge/statut-status.ces--sc.fr-22c55e?style=for-the-badge" alt="Statut des services"></a>
</p>

> **Réservée aux membres du CES et aux invités autorisés.** La connexion se fait avec votre compte du portail
> [app.ces-sc.fr](https://app.ces-sc.fr). Version actuelle : **canary** (version d'essai, mises à jour fréquentes).

---

## Ce qu'elle fait

| | |
|---|---|
| 🛰️ **Le portail du CES, intégré** | Hub, opérations, messagerie, forum, annuaire, hangar, académie, planning : le portail complet, sans navigateur et sans se reconnecter. |
| 🎮 **Présence en jeu** | Le portail sait quand vous êtes en jeu (si vous l'autorisez) et annonce votre arrivée à l'escadre. |
| 📓 **Carnet de voyage** | Vos sessions, sauts quantiques, lieux visités et missions, tirés du journal du jeu. Avec une **carte stellaire** de vos trajets entre Stanton, Pyro et Nyx. |
| 🔔 **Notifications Windows** | Lancements de membres, alertes **DEFCON** de l'organisation, notifications du portail. Tout se règle et se coupe. |
| 🇫🇷 **Traduction française** | Installe et met à jour la traduction communautaire du jeu, avec sauvegarde. |
| ⌨️ **Profils de touches** | Sauvegarde, restauration et partage de vos réglages de commandes avec l'escadre. |
| 💰 **Commerce, minage, Wikelo** | Routes commerciales (données UEX), suivi des sessions de minage avec lecture de l'écran de raffinerie, contrats de Wikelo. |

## Installation

1. Téléchargez `CES-Compagnon_<version>_x64-setup.exe` dans la release
   [**canary**](https://github.com/Kalgiio/ces-compagnon-releases/releases/tag/canary).
2. Lancez-le : l'installation se fait pour votre seul compte Windows, **sans droits administrateur**.
3. Si Windows affiche « Windows a protégé votre ordinateur » (l'application n'a pas encore de certificat de
   signature de code) : **Informations complémentaires → Exécuter quand même**.
4. Au premier lancement, cliquez sur **Se connecter** : le portail s'ouvre dans votre navigateur, vous vous y
   connectez comme d'habitude et vous approuvez l'appareil. Aucun mot de passe n'est saisi dans l'application.

Les mises à jour suivantes sont proposées **dans l'application**. Chaque paquet est signé et sa signature est
vérifiée avant installation.

**Configuration :** Windows 10 ou 11, 64 bits. Star Citizen installé (dossier détecté automatiquement, ou choisi à
la main).

## Votre jeu et vos données

- **L'application ne touche pas au jeu.** Elle regarde si Star Citizen est lancé et lit les fichiers que le jeu écrit
  (journal, réglages, touches), comme un simple lecteur de fichiers. Aucune lecture de la mémoire du jeu, aucune
  injection, aucune interception réseau, aucune automatisation. Elle n'écrit dans le dossier du jeu que sur votre
  demande (traduction, touches), avec une sauvegarde.
- **Vous choisissez ce qui est partagé**, type par type. Seuls **vos propres** événements partent vers le portail :
  jamais le journal brut, jamais d'information sur les autres joueurs.
- Votre jeton de connexion est rangé dans le **gestionnaire d'identification de Windows**, jamais dans un fichier.

## Remerciements

- Traduction française : [SCEFRA](https://github.com/SPEED0U/Scefra) (CC BY-NC-ND 4.0), installée telle quelle.
- Prix, terminaux et raffineries : [UEX](https://uexcorp.space) — données communautaires, sans garantie.
- Contrats de Wikelo : [Star Citizen Wiki](https://star-citizen.wiki).

## Ce dépôt

Ce dépôt contient **seulement les versions publiées** de l'application, pas son code source.
Un problème, une idée ? Passez par le Discord du CES.

---

<sub>Outil de fans **non affilié à Cloud Imperium Games**. Star Citizen®, Squadron 42®, Roberts Space Industries® et
Cloud Imperium® sont des marques de Cloud Imperium Rights LLC et Cloud Imperium Rights Ltd. *Made by the Community.*</sub>
