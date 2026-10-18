; Yaca's own steps in the uninstaller, on top of Tauri's NSIS template.
;
; Tauri's "Delete the application data" checkbox removes the folders named
; after the bundle identifier (com.yaca.desktop), which hold the webview
; profile. Yaca keeps its settings and chats in %LOCALAPPDATA%\Yaca
; (registry::config_dir and store::location in yaca-core) and provider keys in
; the Windows Credential Manager, so the checkbox is extended here to both.
;
; Saved as UTF-8 with a BOM: without it NSIS reads the Russian text below in
; the system's ANSI code page.

!macro NSIS_HOOK_PREUNINSTALL
  ; Asked before anything is removed. "No" turns the checkbox off, so the
  ; uninstall goes on and keeps the data, Tauri's part of it included.
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    Push $0
    Push $1
    ; The installer's own language is English only, so the warning follows
    ; the language of Windows instead. 0x19 is the primary id of Russian.
    System::Call 'kernel32::GetUserDefaultUILanguage() i .r0'
    IntOp $0 $0 & 0x3FF
    ${If} $0 = 0x19
      StrCpy $1 "Все чаты и настройки Yaca будут удалены безвозвратно:$\r$\n$LOCALAPPDATA\Yaca$\r$\n$\r$\nКлючи провайдеров тоже будут удалены из диспетчера учётных данных Windows. Папки .yaca внутри проектов останутся.$\r$\n$\r$\nУдалить всё?"
    ${Else}
      StrCpy $1 "All Yaca chats and settings will be permanently deleted:$\r$\n$LOCALAPPDATA\Yaca$\r$\n$\r$\nProvider keys will be removed from Windows Credential Manager too. The .yaca folders inside projects are kept.$\r$\n$\r$\nDelete everything?"
    ${EndIf}
    MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON2 "$1" /SD IDNO IDYES +2
    StrCpy $DeleteAppDataCheckboxState 0
    Pop $1
    Pop $0
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    SetShellVarContext current
    ; The folder name is CONFIG_DIR in yaca-core, not the product name: it
    ; must not move if the installer is ever renamed.
    RMDir /r "$LOCALAPPDATA\Yaca"
    ; Builds before 1.0.3 kept settings in Roaming.
    RMDir /r "$APPDATA\Yaca"
    ; Keys are stored as "<provider>.Yaca" (and "<provider>.edoC" by the build
    ; before the rename). Matched case-sensitively and to the end of the name,
    ; so that another program's entry that merely contains "yaca" survives.
    nsExec::Exec `powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "cmdkey /list | ForEach-Object { if ($$_ -cmatch 'target=(\S+\.(Yaca|edoC))\s*$$') { cmdkey /delete:$$($$Matches[1]) | Out-Null } }"`
    Pop $0
  ${EndIf}
!macroend
