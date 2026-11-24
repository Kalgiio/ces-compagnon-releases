; Habillage de l'installeur NSIS (maquette de le CES : docs/companion/design/installation-maquette.png).
; Ce fichier est inclus par le modèle NSIS de Tauri AVANT les pages MUI2 : les définitions ci-dessous remplacent
; donc les valeurs par défaut de MUI2 (qui ne les pose que si elles n'existent pas).
; Limite de NSIS : les pages d'accueil et de fin et le bandeau supérieur prennent ces couleurs ; les pages
; intermédiaires (dossier, progression) gardent le fond du système. Image latérale : installer/sidebar.bmp.

; Fond nuit et texte clair (accueil, fin, bandeau supérieur).
!define MUI_BGCOLOR "0B0D1E"
!define MUI_TEXTCOLOR "E6E8F2"
; Journal d'installation : texte clair sur fond nuit.
!define MUI_INSTFILESPAGE_COLORS "E6E8F2 0B0D1E"
!define MUI_INSTFILESPAGE_PROGRESSBAR "colored"

; Textes de la page d'accueil (maquette).
!define MUI_WELCOMEPAGE_TITLE "Bienvenue dans le programme d'installation de CES Compagnon"
!define MUI_WELCOMEPAGE_TEXT "Vous êtes sur le point d'installer CES Compagnon sur votre ordinateur.$\r$\n$\r$\nAvant de démarrer l'installation, il est recommandé de fermer toutes les autres applications. Cela permettra la mise à jour de certains fichiers système sans redémarrer votre ordinateur.$\r$\n$\r$\nCliquez sur Suivant pour continuer."
!define MUI_FINISHPAGE_TITLE "CES Compagnon est installé"
