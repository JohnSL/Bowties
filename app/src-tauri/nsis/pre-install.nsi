; Pre-install macro: check if Bowties is running and offer to close it
; before the installer replaces any files.
!macro preInstall
  FindWindow $0 "" "Bowties::LCC"
  IntCmp $0 0 notRunning
    MessageBox MB_OKCANCEL|MB_ICONEXCLAMATION \
      "Bowties is currently running.$\nClick OK to close it and continue, or Cancel to abort the installation." \
      IDOK killApp IDCANCEL abort
    killApp:
      nsExec::Exec '"taskkill" /F /IM "bowties.exe" /T'
      Pop $0
      Sleep 1000
      Goto notRunning
    abort:
      Abort
  notRunning:
!macroend
