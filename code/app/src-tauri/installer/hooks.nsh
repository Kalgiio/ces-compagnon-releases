; Habillage de l'installeur NSIS (maquette de le CES : docs/companion/design/installation-maquette.png).
; Ce fichier est inclus par le modèle NSIS de Tauri AVANT les pages MUI2 : les définitions ci-dessous remplacent
; donc les valeurs par défaut de MUI2 (qui ne les pose que si elles n'existent pas).
; Limite de NSIS : les pages d'accueil et de fin et le bandeau supérieur prennent ces couleurs ; les pages
; intermédiaires (dossier, progression) gardent le fond du système. Image latérale : installer/sidebar.bmp.

; Fond BLANC et texte foncé (le CES, 25/11 : le noir était illisible). Les autres pages sont mises au blanc par le modèle
; installer/installer.nsi (CesPageClaire).
!define MUI_BGCOLOR "FFFFFF"
!define MUI_TEXTCOLOR "1A1B2E"
; Journal d'installation : texte foncé sur fond blanc.
!define MUI_INSTFILESPAGE_COLORS "1A1B2E FFFFFF"
!define MUI_INSTFILESPAGE_PROGRESSBAR "colored"

; Textes de la page d'accueil (maquette).
!define MUI_WELCOMEPAGE_TITLE "Bienvenue dans le programme d'installation de CES Compagnon"
!define MUI_WELCOMEPAGE_TEXT "Vous êtes sur le point d'installer CES Compagnon sur votre ordinateur.$\r$\n$\r$\nAvant de démarrer l'installation, il est recommandé de fermer toutes les autres applications. Cela permettra la mise à jour de certains fichiers système sans redémarrer votre ordinateur.$\r$\n$\r$\nCliquez sur Suivant pour continuer."
!define MUI_FINISHPAGE_TITLE "CES Compagnon est installé"

; Page « Conditions » (installer/conditions.rtf, texte foncé) : fond blanc, et acceptation obligatoire.
!define MUI_LICENSEPAGE_BGCOLOR "FFFFFF"
!define MUI_LICENSEPAGE_TEXT_TOP "Conditions d'utilisation et données personnelles de CES Compagnon"

; Conditions acceptées sur la page « Conditions » : l'application ne les redemande pas. Seulement pour une installation
; normale (les mises à jour automatiques sont passives et ne montrent pas la page). Version = conditions.rs (VERSION).
!macro NSIS_HOOK_POSTINSTALL
  ${If} $PassiveMode <> 1
  ${AndIfNot} ${Silent}
    CreateDirectory "$APPDATA\fr.ces-sc.compagnon"
    FileOpen $0 "$APPDATA\fr.ces-sc.compagnon\conditions-acceptees.txt" w
    FileWrite $0 "2026-11-25|installeur|installeur$\r$\n"
    FileClose $0
  ${EndIf}
!macroend
