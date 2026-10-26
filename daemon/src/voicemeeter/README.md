# Модуль Voicemeeter

Связывает фейдеры и кнопки GoXLR с Voicemeeter (Basic / Banana / Potato) в обе стороны:
двигаете фейдер на GoXLR — двигается strip/bus в Voicemeeter, и наоборот.

В интерфейсе появляется вкладка **Voicemeeter** между «Маршрутизация» и «Система».

## Что можно связать

| GoXLR | Voicemeeter | Направления |
|---|---|---|
| Физический фейдер A–D (канал, который сейчас на нём) | `Strip[n].Gain`, `Bus[n].Gain` или любой параметр | ⇄ / → / ← |
| Громкость канала (Mic, Music, Chat, …) | то же | ⇄ / → / ← |
| Мьют фейдера A–D, мьют микрофона (Cough) | `Strip[n].Mute/Solo/Mono/A1…B3`, `Bus[n].Mute/Mono/Sel/EQ.on`, `MacroButton[n].State/StateOnly/Trigger`, любой параметр | ⇄ / → / ← |
| FX, Megaphone, Robot, Hard Tune | то же | ⇄ / → / ← |
| Пресет эффектов 1–6, банк сэмплера A–C | то же (включение в VM выбирает пресет/банк) | ⇄ / → / ← |
| Удержание любой физической кнопки | то же | только → |

Громкость переводится линейно в дБ: 0% GoXLR = «Мин, дБ», 100% = «Макс, дБ» (по умолчанию −60…0).
«Инверсия» для кнопок: параметр VM включён, когда состояние GoXLR выключено.

## Профили

Для каждого профиля GoXLR `Profiles/<имя>.goxlr` хранится отдельный файл
`Profiles/Voicemeeter/<имя>.json`. Оригинальные профили GoXLR не изменяются.

- **Сохранить / Сохранить как** профиль GoXLR — сохраняется и профиль Voicemeeter.
- **Загрузить** профиль GoXLR — загружается соответствующий профиль Voicemeeter
  (если файла нет — создаётся пустой).
- **Новый** профиль — создаётся пустой профиль Voicemeeter.
- **Удалить** профиль GoXLR — удаляется и файл Voicemeeter.
- Во вкладке Voicemeeter есть свои кнопки «Сохранить» и «Отменить изменения».

Формат файла:

```json
{
  "enabled": true,
  "faders": [
    { "source": { "Fader": "A" }, "target": "Strip[0].Gain", "direction": "Both", "min_db": -60.0, "max_db": 0.0 }
  ],
  "buttons": [
    { "source": "FxEnabled", "target": "Bus[5].Mute", "direction": "ToVoicemeeter", "invert": true }
  ]
}
```

`direction`: `Both`, `ToVoicemeeter`, `FromVoicemeeter`.

## Как устроено

| Файл | Назначение |
|---|---|
| `mod.rs` | `spawn()`: прокси канала команд (ловит операции с профилями) и рабочая задача синхронизации |
| `links.rs` | Движок синхронизации, защита от «эха» и отскока значений при перетаскивании |
| `remote.rs` | Работа с `VoicemeeterRemote64.dll` в отдельном потоке (только Windows) |
| `config.rs` | Формат профиля и чтение/запись файлов |
| `http.rs` | REST API `/api/voicemeeter/*`, раздача UI, подключение скрипта к `index.html` |
| `ui/voicemeeter.js`, `ui/voicemeeter.css` | Вкладка в интерфейсе |

DLL ищется по пути установки Voicemeeter из реестра. Свой путь можно задать переменной окружения
`VOICEMEETER_REMOTE_DLL`.

Веб-интерфейс GoXLR Utility поставляется уже собранным (`daemon/web-content`), поэтому модуль его
**не изменяет**: при отдаче `index.html` демон добавляет в него `<script src="/voicemeeter/voicemeeter.js">`,
а скрипт сам встраивает вкладку в список вкладок устройства.

## Точки подключения к основному коду

Всего три места (10 строк):

1. `daemon/src/main.rs` — объявление модуля:
   ```rust
   mod voicemeeter;
   ```
2. `daemon/src/main.rs` — сразу после `let _ = ready_rx.await;`:
   ```rust
   // Voicemeeter Module, wraps the device command channel used by the servers below.
   let usb_tx = voicemeeter::spawn(
       usb_tx,
       broadcast_tx.clone(),
       settings.clone(),
       shutdown.clone(),
   );
   ```
3. `daemon/src/servers/http_server.rs` — перед `.default_service(...)`:
   ```rust
   .configure(crate::voicemeeter::http::configure)
   ```

Новые зависимости не нужны (используются уже подключённые `windows`, `winreg`, `actix-web`).

## Обновление с версии автора

```sh
git fetch upstream
git merge upstream/main
```

Каталог модуля upstream не трогает, поэтому конфликт возможен только в трёх строках выше.
Обновление `daemon/web-content` от автора тоже ничего не ломает: скрипт подключается на лету.

## Удаление модуля

Удалить каталог `daemon/src/voicemeeter/` и три строки из раздела выше. Файлы
`Profiles/Voicemeeter/*.json` можно удалить вручную.

## Тесты

```sh
cargo test -p goxlr-daemon voicemeeter
```
