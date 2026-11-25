"""Construit src-tauri/installer/installer.nsi (modèle NSIS de CES Compagnon) depuis le modèle officiel de Tauri.

Usage : python scripts/modele-installeur.py <installer.nsi officiel de la version de tauri-cli utilisée>
  Le modèle officiel : https://raw.githubusercontent.com/tauri-apps/tauri/tauri-cli-v<version>/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi
  À REFAIRE à chaque mise à jour de tauri-cli (le modèle et l'outil doivent aller ensemble).

Changements (demande de Luka, 25/11) :
- plus de case « Créer un raccourci sur le bureau » à la fin (le menu Démarrer reste) ;
- TOUTES les pages sur fond BLANC et texte foncé (Luka, 25/11 : le noir était illisible) ;
- la page « Conditions » (licence) vient de bundle.licenseFile (installer/conditions.rtf).
"""
import os
import sys

FOND, TEXTE = 'FFFFFF', '1A1B2E'

SOMBRE = r'''
; --- CES Compagnon : pages sur fond blanc, texte foncé (toutes les pages, pas seulement l'accueil et la fin) -------
; Colore la fenêtre, le bandeau du haut et chaque contrôle de la page affichée. Les cases et boutons radio perdent le
; thème visuel de Windows (pour prendre ces couleurs) ; les boutons poussoirs gardent leur aspect système.
!macro CES_PAGE_CLAIRE UN
Function ${UN}CesPageClaire
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  SetCtlColors $HWNDPARENT 0x__TEXTE__ 0x__FOND__
  ${ForEach} $0 1034 1039 + 1
    GetDlgItem $1 $HWNDPARENT $0
    SetCtlColors $1 0x__TEXTE__ 0x__FOND__
  ${Next}
  GetDlgItem $1 $HWNDPARENT 1028
  SetCtlColors $1 0x__TEXTE__ 0x__FOND__
  GetDlgItem $1 $HWNDPARENT 1256
  SetCtlColors $1 0x__TEXTE__ 0x__FOND__
  FindWindow $1 "#32770" "" $HWNDPARENT
  SetCtlColors $1 0x__TEXTE__ 0x__FOND__
  StrCpy $2 0
  ${Do}
    FindWindow $2 "" "" $1 $2
    ${If} $2 = 0
      ${Break}
    ${EndIf}
    System::Call 'user32::GetClassNameW(p r2, w .r3, i 64)'
    ${If} $3 == "Button"
      System::Call 'user32::GetWindowLongW(p r2, i -16) i .r4'
      IntOp $4 $4 & 0xF
      ${If} $4 > 1
        System::Call 'uxtheme::SetWindowTheme(p r2, w " ", w " ")'
        SetCtlColors $2 0x__TEXTE__ 0x__FOND__
      ${EndIf}
    ${ElseIf} $3 == "Static"
    ${OrIf} $3 == "Edit"
      SetCtlColors $2 0x__TEXTE__ 0x__FOND__
    ${EndIf}
  ${Loop}
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
FunctionEnd
!macroend
!insertmacro CES_PAGE_CLAIRE ""
!insertmacro CES_PAGE_CLAIRE "un."
'''.replace('__FOND__', FOND).replace('__TEXTE__', TEXTE)


def remplacer(s, a, b, n=1):
    assert s.count(a) >= n, a[:80]
    return s.replace(a, b, n)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    s = open(sys.argv[1], encoding='utf-8').read()
    s = remplacer(s, '; Installer pages, must be ordered as they appear', SOMBRE + '\n; Installer pages, must be ordered as they appear')
    # Licence (conditions), dossier, installation : fond blanc à l'affichage.
    s = remplacer(s, '  !insertmacro MUI_PAGE_LICENSE "${LICENSE}"', '  !define MUI_PAGE_CUSTOMFUNCTION_SHOW CesPageClaire\n  !insertmacro MUI_PAGE_LICENSE "${LICENSE}"')
    s = remplacer(s, '!insertmacro MUI_PAGE_DIRECTORY', '!define MUI_PAGE_CUSTOMFUNCTION_SHOW CesPageClaire\n!insertmacro MUI_PAGE_DIRECTORY')
    s = remplacer(s, '!insertmacro MUI_PAGE_INSTFILES', '!define MUI_PAGE_CUSTOMFUNCTION_SHOW CesPageClaire\n!insertmacro MUI_PAGE_INSTFILES')
    s = remplacer(s, '!insertmacro MUI_UNPAGE_INSTFILES', '!define MUI_PAGE_CUSTOMFUNCTION_SHOW un.CesPageClaire\n!insertmacro MUI_UNPAGE_INSTFILES')
    # Page « réinstaller / désinstaller » (nsDialogs) et confirmation de désinstallation.
    s = remplacer(s, '    nsDialogs::Show', '    Call CesPageClaire\n    nsDialogs::Show')
    s = remplacer(s, '  SendMessage $DeleteAppDataCheckbox ${WM_SETFONT} $1 1\nFunctionEnd',
                  '  SendMessage $DeleteAppDataCheckbox ${WM_SETFONT} $1 1\n  Call un.CesPageClaire\nFunctionEnd')
    # Plus de case « raccourci sur le bureau » sur la page de fin.
    s = remplacer(s, '; Use show readme button in the finish page as a button create a desktop shortcut\n'
                     '!define MUI_FINISHPAGE_SHOWREADME\n'
                     '!define MUI_FINISHPAGE_SHOWREADME_TEXT "$(createDesktop)"\n'
                     '!define MUI_FINISHPAGE_SHOWREADME_FUNCTION CreateOrUpdateDesktopShortcut\n',
                  '; CES Compagnon : pas de case « raccourci sur le bureau » (demande de Luka, 25/11).\n')
    dest = os.path.join(os.path.dirname(__file__), '..', 'src-tauri', 'installer', 'installer.nsi')
    open(dest, 'w', encoding='utf-8', newline='\n').write(s)
    print('installer/installer.nsi écrit')


if __name__ == '__main__':
    main()
