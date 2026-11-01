; Cam Dock kurulum kancaları (Tauri NSIS)
; - Kurulumda anonim kullanım istatistiği izni sorulur (varsayılan: Evet).
;   Seçim kayıt defterine yazılır; uygulama ilk açılışta okuyup ayarlara işler.
;   Sessiz kurulumda (otomatik güncelleme) sorulmaz, mevcut ayar korunur.
; - Kaldırırken, izin açıksa uygulama "kaldırıldı" bildirimini gönderir.

!macro NSIS_HOOK_POSTINSTALL
  IfSilent camdock_tel_done
  MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON1 "Cam Dock'u geliştirebilmemiz için anonim kullanım istatistiği gönderilsin mi?$\r$\n$\r$\nGönderilenler: rastgele bir kurulum kimliği, uygulama ve Windows sürümü, ekran sayısı, açık widget'lar ve ülke.$\r$\nIP adresi, bilgisayar adı ya da kullanıcı adı gönderilmez.$\r$\n$\r$\nİstediğin zaman Ayarlar → Hakkında bölümünden kapatabilir ve verilerini sildirebilirsin." IDYES camdock_tel_yes
  WriteRegDWORD HKCU "Software\CamDock" "Telemetry" 0
  Goto camdock_tel_mark
  camdock_tel_yes:
  WriteRegDWORD HKCU "Software\CamDock" "Telemetry" 1
  camdock_tel_mark:
  WriteRegDWORD HKCU "Software\CamDock" "TelemetryPending" 1
  camdock_tel_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall-ping'
  ; Gezgin sağ tık menüsü ve Windows ile başlat kaydı
  DeleteRegKey HKCU "Software\Classes\exefile\shell\CamDockPin"
  DeleteRegKey HKCU "Software\Classes\lnkfile\shell\CamDockPin"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "CamDock"
!macroend
