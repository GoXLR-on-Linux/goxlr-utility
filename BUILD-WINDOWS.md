# Сборка GoXLR Utility под Windows

Инструкция повторяет шаги официального CI (`.github/workflows/build-release.yml`, job `build_windows`).
Общая инструкция автора — в [wiki](https://github.com/GoXLR-on-Linux/goxlr-utility/wiki/Compilation-Guide).

## 1. Что установить

| Что | Зачем | Как |
|---|---|---|
| Visual Studio 2022 или Build Tools 2022, компонент «Разработка классических приложений на C++» (MSVC + Windows SDK) | компилятор и линкер | `winget install Microsoft.VisualStudio.2022.BuildTools` и в установщике отметить «Desktop development with C++» |
| Rust (stable, `x86_64-pc-windows-msvc`) | сборка | `winget install Rustlang.Rustup`, затем `rustup default stable` |
| Git | исходники | `winget install Git.Git` |
| Драйвер GoXLR от TC-Helicon | без него демон не запускается (только для запуска, для сборки не нужен) | с сайта TC-Helicon |
| NSIS 3.x | только для сборки установщика | `winget install NSIS.NSIS` |

После установки Rust откройте новый терминал, чтобы `cargo` появился в `PATH`
(или используйте `%USERPROFILE%\.cargo\bin\cargo.exe`).

## 2. Сборка утилиты

```powershell
cd goxlr-utility
cargo build --release --all-features
```

`--all-features` включает озвучку (TTS) для скринридеров, как в официальных сборках. Без этого флага всё
тоже работает, только без TTS.

Результат лежит в `target\release\`:

| Файл | Что это |
|---|---|
| `goxlr-daemon.exe` | демон: работа с GoXLR, веб-интерфейс, модуль Voicemeeter |
| `goxlr-launcher.exe` | запускает демон и открывает интерфейс |
| `goxlr-client.exe`, `goxlr-client-quiet.exe` | управление из командной строки |
| `goxlr-defaults.exe` | извлечение стандартных профилей |
| `SAAPI64.dll`, `nvdaControllerClient64.dll` | библиотеки TTS (только с `--all-features`) |

## 3. Запуск без установщика

Перед запуском закройте официальное приложение GoXLR App и уже установленную GoXLR Utility (если она есть):
демон не стартует, если запущены они.

```powershell
.\target\release\goxlr-daemon.exe
```

Интерфейс откроется в браузере по адресу <http://localhost:14564/>; вкладка **Voicemeeter** — между
«Маршрутизация» и «Система».

Полезные параметры демона:

```powershell
.\target\release\goxlr-daemon.exe --log-level debug     # подробный лог
.\target\release\goxlr-daemon.exe --http-port 14600      # другой порт
.\target\release\goxlr-daemon.exe --help                 # все параметры
```

Логи: `%APPDATA%\GoXLR-on-Linux\GoXLR-Utility\data\logs\goxlr-daemon.log`.

### Замена уже установленной версии

Проще всего: закрыть установленную утилиту (иконка в трее → Quit) и скопировать собранные `.exe` (и `.dll`)
поверх файлов в папке установки (обычно `C:\Program Files\GoXLR Utility\`).

## 4. Оконное приложение (необязательно)

Установщик автора кладёт ещё `goxlr-utility-ui.exe` — окно на WebView2 вместо браузера. Оно собирается из
отдельного репозитория в ту же папку `target`:

```powershell
cd ..
git clone https://github.com/goxlr-on-linux/goxlr-utility-ui-wrapper-app goxlr-utility-ui
cargo build --manifest-path=goxlr-utility-ui\src-tauri\Cargo.toml --target-dir=goxlr-utility\target --release
```

Этот репозиторий модуль Voicemeeter не трогает: окно показывает тот же интерфейс демона.

## 5. Установщик (необязательно)

Нужны собранные утилита (шаг 2) и оконное приложение (шаг 4), а также:

1. Плагин `nsis_tauri_utils.dll`
   ([скачать](https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.1.1/nsis_tauri_utils.dll))
   → в `<NSIS>\Plugins\x86-unicode\`.
2. `MicrosoftEdgeWebview2Setup.exe` ([скачать](https://go.microsoft.com/fwlink/p/?LinkId=2124703)) → в `ci\nsis\`.
3. `vc_redist.x64.exe` ([скачать](https://aka.ms/vs/17/release/vc_redist.x64.exe)) → в `ci\nsis\vc_redist.exe`.

```powershell
cd goxlr-utility\ci\nsis
& "C:\Program Files (x86)\NSIS\makensis.exe" /DPRODUCT_VERSION=1.2.4 goxlr-utility.nsi
```

Готовый установщик: `ci\Output\goxlr-utility-1.2.4.exe`.

## 6. Тесты

```powershell
cargo test -p goxlr-daemon voicemeeter    # тесты модуля Voicemeeter
cargo test                                 # все тесты
cargo clippy --all-targets                 # линтер, как в CI автора
```

## Частые проблемы

- **`link.exe not found` / `cannot find Windows SDK`** — не установлен компонент C++ в Visual Studio Build Tools.
- **`cargo: command not found`** — откройте новый терминал после установки Rust.
- **«The GoXLR Driver was not found»** при запуске — не установлен драйвер TC-Helicon.
- **«The official GoXLR Application is currently running»** — закройте GoXLR App.
- **`failed to remove file ...goxlr-daemon.exe`** при сборке — демон ещё запущен, закройте его.
- **Вкладка Voicemeeter пишет «Voicemeeter Remote API не найден»** — Voicemeeter не установлен или стоит в
  нестандартной папке; путь к `VoicemeeterRemote64.dll` можно задать переменной окружения
  `VOICEMEETER_REMOTE_DLL` перед запуском демона.
