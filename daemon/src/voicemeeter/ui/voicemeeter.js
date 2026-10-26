/*
 * Voicemeeter tab for the GoXLR Utility UI.
 *
 * The upstream UI is a pre-built Vue bundle, so instead of modifying it this script is injected
 * by the daemon, and adds a 'Voicemeeter' tab (between Routing and System) to the device tabs.
 * All data comes from /api/voicemeeter/*, see daemon/src/voicemeeter/.
 */
(() => {
    'use strict';

    const API = '/api/voicemeeter';
    const POLL_ACTIVE = 1000;
    const PUSH_DELAY = 250;

    const CHANNELS = ['Mic', 'LineIn', 'Console', 'System', 'Game', 'Chat', 'Sample', 'Music', 'Headphones', 'MicMonitor', 'LineOut'];
    const FADERS = ['A', 'B', 'C', 'D'];
    const PRESETS = ['Preset1', 'Preset2', 'Preset3', 'Preset4', 'Preset5', 'Preset6'];
    const BANKS = ['A', 'B', 'C'];
    const MINI_BUTTONS = ['Fader1Mute', 'Fader2Mute', 'Fader3Mute', 'Fader4Mute', 'Bleep', 'Cough'];
    const FULL_BUTTONS = [
        'EffectSelect1', 'EffectSelect2', 'EffectSelect3', 'EffectSelect4', 'EffectSelect5', 'EffectSelect6',
        'EffectFx', 'EffectMegaphone', 'EffectRobot', 'EffectHardTune',
        'SamplerSelectA', 'SamplerSelectB', 'SamplerSelectC',
        'SamplerTopLeft', 'SamplerTopRight', 'SamplerBottomLeft', 'SamplerBottomRight', 'SamplerClear',
    ];
    const MACRO_MODES = ['State', 'StateOnly', 'Trigger'];

    const STRINGS = {
        en: {
            tab: 'Voicemeeter',
            connected: 'connected',
            notRunning: 'Voicemeeter is not running',
            notInstalled: 'Voicemeeter Remote API not found',
            unsupported: 'Voicemeeter is only available on Windows',
            noDevice: 'No GoXLR connected',
            device: 'Device',
            profile: 'Profile',
            unsaved: 'unsaved changes',
            enabled: 'Sync enabled',
            save: 'Save',
            revert: 'Revert',
            faders: 'Faders and Volumes',
            buttons: 'Buttons',
            source: 'GoXLR',
            direction: 'Direction',
            target: 'Voicemeeter',
            minDb: 'Min, dB',
            maxDb: 'Max, dB',
            invert: 'Invert',
            now: 'Now',
            add: '+ Add link',
            remove: 'Remove link',
            empty: 'No links yet',
            both: '⇄ Both ways',
            toVm: '→ to Voicemeeter',
            fromVm: '← from Voicemeeter',
            groupFaders: 'Physical faders',
            groupChannels: 'Channels',
            groupMutes: 'Fader mute buttons',
            groupMic: 'Microphone',
            groupEffects: 'Effects',
            groupPresets: 'Effect presets',
            groupBanks: 'Sampler banks',
            groupHeld: 'Button held (GoXLR → Voicemeeter only)',
            fader: 'Fader',
            faderMute: 'Mute:',
            cough: 'Mic mute (Cough)',
            fx: 'FX enabled',
            megaphone: 'Megaphone',
            robot: 'Robot',
            hardTune: 'Hard Tune',
            preset: 'Preset',
            bank: 'Sampler bank',
            strip: 'Strip',
            bus: 'Bus',
            macro: 'Macro button',
            custom: 'Other parameter…',
            customHint: 'e.g. Strip[0].Gain',
            virtual: 'virtual',
            input: 'Input',
            saved: 'Saved',
            help: [
                'Links are stored per GoXLR profile in a separate file, and are saved together with the GoXLR profile (Save / Save As in the profile menu), or with the Save button above.',
                'Volume: 0% on the GoXLR equals "Min, dB" in Voicemeeter, 100% equals "Max, dB" (linear in dB).',
                'Invert: the Voicemeeter parameter is on when the GoXLR state is off (e.g. FX on → mute a strip).',
            ],
            channels: {
                Mic: 'Mic', LineIn: 'Line In', Console: 'Console', System: 'System', Game: 'Game', Chat: 'Chat',
                Sample: 'Sample', Music: 'Music', Headphones: 'Headphones', MicMonitor: 'Mic Monitor', LineOut: 'Line Out',
            },
            params: {Mute: 'Mute', Solo: 'Solo', Mono: 'Mono', Sel: 'Select', 'EQ.on': 'EQ on'},
            macroModes: {State: 'press / release', StateOnly: 'state only (no action)', Trigger: 'trigger'},
        },
        ru: {
            tab: 'Voicemeeter',
            connected: 'подключён',
            notRunning: 'Voicemeeter не запущен',
            notInstalled: 'Voicemeeter Remote API не найден',
            unsupported: 'Voicemeeter доступен только в Windows',
            noDevice: 'GoXLR не подключён',
            device: 'Устройство',
            profile: 'Профиль',
            unsaved: 'есть несохранённые изменения',
            enabled: 'Синхронизация включена',
            save: 'Сохранить',
            revert: 'Отменить изменения',
            faders: 'Фейдеры и громкость',
            buttons: 'Кнопки',
            source: 'GoXLR',
            direction: 'Направление',
            target: 'Voicemeeter',
            minDb: 'Мин, дБ',
            maxDb: 'Макс, дБ',
            invert: 'Инверсия',
            now: 'Сейчас',
            add: '+ Добавить связь',
            remove: 'Удалить связь',
            empty: 'Связей пока нет',
            both: '⇄ В обе стороны',
            toVm: '→ в Voicemeeter',
            fromVm: '← из Voicemeeter',
            groupFaders: 'Физические фейдеры',
            groupChannels: 'Каналы',
            groupMutes: 'Кнопки мьюта фейдеров',
            groupMic: 'Микрофон',
            groupEffects: 'Эффекты',
            groupPresets: 'Пресеты эффектов',
            groupBanks: 'Банки сэмплера',
            groupHeld: 'Удержание кнопки (только GoXLR → Voicemeeter)',
            fader: 'Фейдер',
            faderMute: 'Мьют:',
            cough: 'Мьют микрофона (Cough)',
            fx: 'FX включены',
            megaphone: 'Мегафон',
            robot: 'Робот',
            hardTune: 'Hard Tune',
            preset: 'Пресет',
            bank: 'Банк сэмплера',
            strip: 'Strip',
            bus: 'Bus',
            macro: 'Макро-кнопка',
            custom: 'Другой параметр…',
            customHint: 'например Strip[0].Gain',
            virtual: 'виртуальный',
            input: 'Вход',
            saved: 'Сохранено',
            help: [
                'Связи хранятся для каждого профиля GoXLR в отдельном файле и сохраняются вместе с профилем GoXLR («Сохранить» / «Сохранить как» в меню профиля) либо кнопкой «Сохранить» выше.',
                'Громкость: 0% на GoXLR соответствует «Мин, дБ» в Voicemeeter, 100% — «Макс, дБ» (линейно в дБ).',
                'Инверсия: параметр Voicemeeter включён, когда состояние GoXLR выключено (например, FX включены → мьют канала).',
            ],
            channels: {
                Mic: 'Микрофон', LineIn: 'Линейный вход', Console: 'Консоль', System: 'Система', Game: 'Игра', Chat: 'Чат',
                Sample: 'Сэмплер', Music: 'Музыка', Headphones: 'Наушники', MicMonitor: 'Мониторинг микрофона', LineOut: 'Линейный выход',
            },
            params: {Mute: 'Mute', Solo: 'Solo', Mono: 'Mono', Sel: 'Sel', 'EQ.on': 'EQ вкл.'},
            macroModes: {State: 'нажатие / отпускание', StateOnly: 'только состояние', Trigger: 'триггер'},
        },
    };

    let lang = 'en';
    const t = (key) => STRINGS[lang][key] ?? STRINGS.en[key] ?? key;

    // ---- State ----
    let server = null;          // last response from /state
    let serial = null;          // device being edited
    let config = null;          // local working copy of the profile
    let configKey = null;       // serial|profile the working copy belongs to
    let pushTimer = null;
    let pushing = false;
    let active = false;
    let pollTimer = null;
    let flash = null;

    let tabButton = null;
    let panel = null;
    let restoreButton = null;

    // ---- Helpers ----
    function el(tag, attrs = {}, ...children) {
        const node = document.createElement(tag);
        for (const [key, value] of Object.entries(attrs)) {
            if (value === undefined || value === null || value === false) continue;
            if (key === 'class') node.className = value;
            else if (key.startsWith('on')) node.addEventListener(key.substring(2), value);
            else if (key === 'value') node.value = value;
            else if (key === 'checked' || key === 'disabled' || key === 'selected') node[key] = true;
            else node.setAttribute(key, value === true ? '' : value);
        }
        for (const child of children.flat()) {
            if (child === null || child === undefined || child === false) continue;
            node.append(child instanceof Node ? child : document.createTextNode(String(child)));
        }
        return node;
    }

    function option(value, label, selected) {
        return el('option', {value, selected: value === selected}, label);
    }

    function select(options, value, onChange, attrs = {}) {
        const node = el('select', attrs, options);
        node.value = value;
        node.addEventListener('change', () => onChange(node.value));
        return node;
    }

    async function api(path, body) {
        const init = body === undefined ? {method: 'GET'} : {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify(body),
        };
        const response = await fetch(API + path, init);
        const data = await response.json();
        if (data && data.Error !== undefined) throw new Error(data.Error);
        return data;
    }

    const device = () => (server && serial) ? server.devices[serial] : null;
    const isMini = () => device()?.device_type === 'Mini';
    const key = (s, d) => `${s}|${d?.profile_name}`;

    // ---- Locale ----
    async function loadLocale() {
        try {
            const status = await (await fetch('/api/get-devices')).json();
            const locale = status.config?.locale?.user_locale || status.config?.locale?.system_locale || 'en';
            lang = locale.toLowerCase().startsWith('ru') ? 'ru' : 'en';
        } catch (e) {
            lang = 'en';
        }
        if (tabButton) tabButton.textContent = t('tab');
    }

    // ---- Server sync ----
    async function refresh() {
        try {
            server = await api('/state');
        } catch (e) {
            server = null;
        }

        const serials = server ? Object.keys(server.devices).sort() : [];
        if (!serials.includes(serial)) serial = serials[0] ?? null;

        const d = device();
        const busy = pushing || pushTimer !== null;
        if (!d) {
            config = null;
            configKey = null;
            render();
        } else if (key(serial, d) !== configKey || (!busy && JSON.stringify(d.config) !== JSON.stringify(config))) {
            config = structuredClone(d.config);
            configKey = key(serial, d);
            render();
        } else {
            renderLive();
        }
    }

    function changed(rerender = true) {
        if (rerender) render();
        clearTimeout(pushTimer);
        pushTimer = setTimeout(push, PUSH_DELAY);
    }

    async function push() {
        pushTimer = null;
        if (!serial || !config) return;
        pushing = true;
        try {
            await api(`/config/${encodeURIComponent(serial)}`, config);
        } catch (e) {
            console.warn('Voicemeeter: unable to update config', e);
        }
        pushing = false;
        await refresh();
    }

    async function flush() {
        if (pushTimer !== null) {
            clearTimeout(pushTimer);
            await push();
        }
    }

    async function save() {
        await flush();
        try {
            await api(`/save/${encodeURIComponent(serial)}`, {});
            flash = t('saved');
            setTimeout(() => { flash = null; renderLive(); }, 2000);
        } catch (e) {
            flash = e.message;
        }
        await refresh();
    }

    async function revert() {
        clearTimeout(pushTimer);
        pushTimer = null;
        try {
            await api(`/reload/${encodeURIComponent(serial)}`, {});
        } catch (e) {
            console.warn('Voicemeeter: unable to reload', e);
        }
        configKey = null;
        await refresh();
    }

    // ---- Voicemeeter layout ----
    function vmInfo() {
        // Without a connection (yet), assume Potato so things can still be configured.
        const info = server?.info;
        if (info) return info;
        const channel = (count, physical) => Array.from({length: count}, (_, i) => ({label: '', is_virtual: i >= physical}));
        return {kind: 3, version: '', strips: channel(8, 5), buses: channel(8, 5)};
    }

    function vmName(kind) {
        return {1: 'Voicemeeter', 2: 'Voicemeeter Banana', 3: 'Voicemeeter Potato'}[kind] ?? 'Voicemeeter';
    }

    function stripName(index) {
        const strip = vmInfo().strips[index];
        const base = `${t('strip')} ${index + 1}`;
        const kind = strip?.is_virtual ? t('virtual') : t('input');
        return strip?.label ? `${base} · ${strip.label}` : `${base} (${kind})`;
    }

    function busName(index) {
        const buses = vmInfo().buses;
        const physical = buses.filter((b) => !b.is_virtual).length;
        const name = index < physical ? `A${index + 1}` : `B${index - physical + 1}`;
        const label = buses[index]?.label;
        return label ? `${t('bus')} ${name} · ${label}` : `${t('bus')} ${name}`;
    }

    function stripParams() {
        const buses = vmInfo().buses;
        const physical = buses.filter((b) => !b.is_virtual).length;
        const routes = buses.map((_, i) => i < physical ? `A${i + 1}` : `B${i - physical + 1}`);
        return ['Mute', 'Solo', 'Mono', ...routes];
    }

    function busParams() {
        return vmInfo().kind === 3 ? ['Mute', 'Mono', 'Sel', 'EQ.on'] : ['Mute', 'Mono', 'EQ.on'];
    }

    function parseTarget(target) {
        let match = /^\s*(Strip|Bus)\[(\d+)\]\.(.+?)\s*$/i.exec(target ?? '');
        if (match) {
            return {kind: match[1].toLowerCase() === 'strip' ? 'Strip' : 'Bus', index: Number(match[2]), param: match[3]};
        }
        match = /^\s*MacroButton\[(\d+)\]\.(State|StateOnly|Trigger)\s*$/i.exec(target ?? '');
        if (match) {
            const mode = MACRO_MODES.find((m) => m.toLowerCase() === match[2].toLowerCase());
            return {kind: 'MacroButton', index: Number(match[1]), param: mode};
        }
        return {kind: 'Custom', index: 0, param: target ?? ''};
    }

    // ---- GoXLR sources ----
    const json = (value) => JSON.stringify(value);

    function faderLabel(fader) {
        const channel = device()?.fader_channels?.[fader];
        const name = channel ? ` (${t('channels')[channel] ?? channel})` : '';
        return `${t('fader')} ${fader}${name}`;
    }

    function volumeSources() {
        return [
            el('optgroup', {label: t('groupFaders')}, FADERS.map((f) => option(json({Fader: f}), faderLabel(f)))),
            el('optgroup', {label: t('groupChannels')}, CHANNELS.map((c) => option(json({Channel: c}), t('channels')[c]))),
        ];
    }

    function buttonSources() {
        const groups = [
            el('optgroup', {label: t('groupMutes')}, FADERS.map((f) => option(json({FaderMute: f}), `${t('faderMute')} ${faderLabel(f)}`))),
            el('optgroup', {label: t('groupMic')}, option(json('CoughMute'), t('cough'))),
        ];
        if (!isMini()) {
            groups.push(
                el('optgroup', {label: t('groupEffects')},
                    option(json('FxEnabled'), t('fx')),
                    option(json('Megaphone'), t('megaphone')),
                    option(json('Robot'), t('robot')),
                    option(json('HardTune'), t('hardTune'))),
                el('optgroup', {label: t('groupPresets')}, PRESETS.map((p, i) => option(json({EffectPreset: p}), `${t('preset')} ${i + 1}`))),
                el('optgroup', {label: t('groupBanks')}, BANKS.map((b) => option(json({SamplerBank: b}), `${t('bank')} ${b}`))),
            );
        }
        const buttons = isMini() ? MINI_BUTTONS : [...MINI_BUTTONS, ...FULL_BUTTONS];
        groups.push(el('optgroup', {label: t('groupHeld')}, buttons.map((b) => option(json({ButtonHeld: b}), b))));
        return groups;
    }

    const isHeld = (source) => typeof source === 'object' && source !== null && 'ButtonHeld' in source;

    function directionSelect(link, forceToVm) {
        if (forceToVm && link.direction !== 'ToVoicemeeter') link.direction = 'ToVoicemeeter';
        return select([
            option('Both', t('both')),
            option('ToVoicemeeter', t('toVm')),
            option('FromVoicemeeter', t('fromVm')),
        ], link.direction, (value) => { link.direction = value; changed(false); }, {disabled: forceToVm, class: 'vm-direction'});
    }

    // ---- Target editors ----
    function customInput(link) {
        return el('input', {
            type: 'text', class: 'vm-custom', value: link.target, placeholder: t('customHint'), spellcheck: 'false',
            onchange: (e) => { link.target = e.target.value.trim(); changed(false); },
        });
    }

    function faderTarget(link) {
        const parsed = parseTarget(link.target);
        const isGain = (parsed.kind === 'Strip' || parsed.kind === 'Bus') && parsed.param === 'Gain';
        const current = isGain ? `${parsed.kind}[${parsed.index}].Gain` : 'Custom';
        const info = vmInfo();

        const picker = select([
            el('optgroup', {label: t('strip')}, info.strips.map((_, i) => option(`Strip[${i}].Gain`, stripName(i)))),
            el('optgroup', {label: t('bus')}, info.buses.map((_, i) => option(`Bus[${i}].Gain`, busName(i)))),
            option('Custom', t('custom')),
        ], current, (value) => {
            link.target = value === 'Custom' ? '' : value;
            changed();
        });

        return el('div', {class: 'vm-target'}, picker, current === 'Custom' ? customInput(link) : null);
    }

    function buttonTarget(link) {
        const parsed = parseTarget(link.target);
        const info = vmInfo();
        const channelValue = parsed.kind === 'Strip' || parsed.kind === 'Bus' ? `${parsed.kind}[${parsed.index}]` : parsed.kind;

        const channel = select([
            el('optgroup', {label: t('strip')}, info.strips.map((_, i) => option(`Strip[${i}]`, stripName(i)))),
            el('optgroup', {label: t('bus')}, info.buses.map((_, i) => option(`Bus[${i}]`, busName(i)))),
            option('MacroButton', t('macro')),
            option('Custom', t('custom')),
        ], channelValue, (value) => {
            if (value === 'Custom') link.target = '';
            else if (value === 'MacroButton') link.target = 'MacroButton[0].State';
            else {
                const params = value.startsWith('Strip') ? stripParams() : busParams();
                const param = params.includes(parsed.param) ? parsed.param : 'Mute';
                link.target = `${value}.${param}`;
            }
            changed();
        });

        const parts = [channel];
        if (parsed.kind === 'Strip' || parsed.kind === 'Bus') {
            const params = parsed.kind === 'Strip' ? stripParams() : busParams();
            const names = t('params');
            const list = params.includes(parsed.param) ? params : [...params, parsed.param];
            parts.push(select(list.map((p) => option(p, names[p] ?? p)), parsed.param, (value) => {
                link.target = `${parsed.kind}[${parsed.index}].${value}`;
                changed();
            }, {class: 'vm-param'}));
        } else if (parsed.kind === 'MacroButton') {
            parts.push(el('input', {
                type: 'number', min: 0, max: 79, class: 'vm-number', value: parsed.index,
                onchange: (e) => {
                    const index = Math.max(0, Math.min(79, parseInt(e.target.value, 10) || 0));
                    link.target = `MacroButton[${index}].${parsed.param}`;
                    changed();
                },
            }));
            const modes = t('macroModes');
            parts.push(select(MACRO_MODES.map((m) => option(m, modes[m])), parsed.param, (value) => {
                link.target = `MacroButton[${parsed.index}].${value}`;
                changed();
            }, {class: 'vm-param'}));
        } else {
            parts.push(customInput(link));
        }
        return el('div', {class: 'vm-target'}, parts);
    }

    function numberInput(link, field) {
        return el('input', {
            type: 'number', step: '0.5', min: -60, max: 12, class: 'vm-number', value: link[field],
            onchange: (e) => {
                const value = parseFloat(e.target.value);
                if (!Number.isNaN(value)) link[field] = value;
                changed();
            },
        });
    }

    function removeButton(list, index) {
        return el('button', {
            type: 'button', class: 'vm-remove', title: t('remove'), 'aria-label': t('remove'),
            onclick: () => { list.splice(index, 1); changed(); },
        }, '✕');
    }

    // ---- Rendering ----
    function liveCell(id) {
        return el('td', {class: 'vm-live', 'data-live': id});
    }

    function faderTable() {
        if (config.faders.length === 0) return el('div', {class: 'vm-empty'}, t('empty'));
        return el('table', {class: 'vm-table'},
            el('thead', {}, el('tr', {},
                ['source', 'direction', 'target', 'minDb', 'maxDb', 'now'].map((k) => el('th', {}, t(k))), el('th'))),
            el('tbody', {}, config.faders.map((link, index) => el('tr', {},
                el('td', {}, select(volumeSources(), json(link.source), (value) => { link.source = JSON.parse(value); changed(); })),
                el('td', {}, directionSelect(link, false)),
                el('td', {}, faderTarget(link)),
                el('td', {}, numberInput(link, 'min_db')),
                el('td', {}, numberInput(link, 'max_db')),
                liveCell(`faders.${index}`),
                el('td', {}, removeButton(config.faders, index)),
            ))),
        );
    }

    function buttonTable() {
        if (config.buttons.length === 0) return el('div', {class: 'vm-empty'}, t('empty'));
        return el('table', {class: 'vm-table'},
            el('thead', {}, el('tr', {},
                ['source', 'direction', 'target', 'invert', 'now'].map((k) => el('th', {}, t(k))), el('th'))),
            el('tbody', {}, config.buttons.map((link, index) => el('tr', {},
                el('td', {}, select(buttonSources(), json(link.source), (value) => {
                    link.source = JSON.parse(value);
                    if (isHeld(link.source)) link.direction = 'ToVoicemeeter';
                    changed();
                })),
                el('td', {}, directionSelect(link, isHeld(link.source))),
                el('td', {}, buttonTarget(link)),
                el('td', {class: 'vm-center'}, el('input', {
                    type: 'checkbox', checked: link.invert,
                    onchange: (e) => { link.invert = e.target.checked; changed(false); },
                })),
                liveCell(`buttons.${index}`),
                el('td', {}, removeButton(config.buttons, index)),
            ))),
        );
    }

    function addFader() {
        const used = config.faders.map((l) => l.source?.Fader);
        const fader = FADERS.find((f) => !used.includes(f)) ?? 'A';
        const strip = Math.min(FADERS.indexOf(fader), vmInfo().strips.length - 1);
        config.faders.push({source: {Fader: fader}, target: `Strip[${strip}].Gain`, direction: 'Both', min_db: -60, max_db: 0});
        changed();
    }

    function addButton() {
        config.buttons.push({source: {FaderMute: 'A'}, target: 'Strip[0].Mute', direction: 'Both', invert: false});
        changed();
    }

    function render() {
        if (!panel) return;
        panel.replaceChildren();

        const header = el('div', {class: 'vm-header'},
            el('span', {class: 'vm-status'}, el('span', {class: 'vm-dot', 'data-live': 'dot'}), el('span', {'data-live': 'status'})));
        panel.append(header);

        const d = device();
        if (!d || !config) {
            panel.append(el('div', {class: 'vm-empty'}, server ? t('noDevice') : '…'));
            renderLive();
            return;
        }

        const serials = Object.keys(server.devices).sort();
        if (serials.length > 1) {
            header.append(el('label', {class: 'vm-field'}, `${t('device')}: `,
                select(serials.map((s) => option(s, s)), serial, async (value) => {
                    await flush();
                    serial = value;
                    configKey = null;
                    await refresh();
                })));
        }

        header.append(
            el('span', {class: 'vm-profile'}, `${t('profile')}: `, el('b', {}, d.profile_name),
                el('span', {class: 'vm-dirty', 'data-live': 'dirty'})),
            el('label', {class: 'vm-field'},
                el('input', {type: 'checkbox', checked: config.enabled, onchange: (e) => { config.enabled = e.target.checked; changed(false); }}),
                ` ${t('enabled')}`),
            el('span', {class: 'vm-spacer'}),
            el('span', {class: 'vm-flash', 'data-live': 'flash'}),
            el('button', {type: 'button', class: 'vm-button', onclick: revert}, t('revert')),
            el('button', {type: 'button', class: 'vm-button vm-primary', onclick: save}, t('save')),
        );

        panel.append(el('div', {class: `vm-groups${config.enabled ? '' : ' vm-disabled'}`},
            el('section', {class: 'vm-group'},
                el('div', {class: 'vm-title', role: 'heading', 'aria-level': 2}, t('faders')),
                faderTable(),
                el('button', {type: 'button', class: 'vm-button vm-add', onclick: addFader}, t('add'))),
            el('section', {class: 'vm-group'},
                el('div', {class: 'vm-title', role: 'heading', 'aria-level': 2}, t('buttons')),
                buttonTable(),
                el('button', {type: 'button', class: 'vm-button vm-add', onclick: addButton}, t('add'))),
        ));

        panel.append(el('div', {class: 'vm-help'},
            t('help').map((line) => el('p', {}, line)),
            el('p', {class: 'vm-file', 'data-live': 'file'})));

        renderLive();
    }

    function setLive(name, apply) {
        panel?.querySelectorAll(`[data-live="${name}"]`).forEach(apply);
    }

    function renderLive() {
        if (!panel) return;

        let state = 'err';
        let text = '…';
        if (server) {
            const remote = server.remote;
            if (remote === 'Connected') {
                state = 'ok';
                text = `${vmName(server.info?.kind)} ${server.info?.version ?? ''} — ${t('connected')}`;
            } else if (remote === 'NotRunning') {
                state = 'warn';
                text = t('notRunning');
            } else if (remote === 'Unsupported') {
                text = t('unsupported');
            } else if (remote?.NotInstalled !== undefined) {
                text = `${t('notInstalled')} (${remote.NotInstalled})`;
            }
        }
        setLive('dot', (n) => { n.className = `vm-dot vm-${state}`; });
        setLive('status', (n) => { n.textContent = text; });

        const d = device();
        if (!d) return;
        const dirty = d.dirty || pushTimer !== null || pushing;
        setLive('dirty', (n) => { n.textContent = dirty ? ` ● ${t('unsaved')}` : ''; });
        setLive('flash', (n) => { n.textContent = flash ?? ''; });
        setLive('file', (n) => { n.textContent = d.file ?? ''; });

        const fmtVolume = (v) => v === null || v === undefined ? '—' : `${Math.round(v / 255 * 100)}%`;
        const fmtDb = (v) => v === null || v === undefined ? '—' : `${v.toFixed(1)} dB`;
        const dot = (on) => el('span', {class: `vm-led${on === null || on === undefined ? '' : on ? ' vm-on' : ' vm-off'}`});

        (d.live?.faders ?? []).forEach((live, i) => setLive(`faders.${i}`, (n) => {
            n.replaceChildren(`${fmtVolume(live.goxlr)} · ${fmtDb(live.vm)}`);
        }));
        (d.live?.buttons ?? []).forEach((live, i) => setLive(`buttons.${i}`, (n) => {
            const vm = live.vm === null || live.vm === undefined ? null : live.vm >= 0.5;
            n.replaceChildren(dot(live.goxlr), ' ', dot(vm));
        }));
    }

    // ---- Tab injection ----
    function tabList() {
        return document.querySelector('#main > div > .tab[role="TabList"]');
    }

    function nativeButtons(list) {
        return [...list.querySelectorAll(':scope > button:not(.vm-tab-button)')];
    }

    function nativeDetails(list) {
        return list.parentElement.querySelector(':scope > .tabs-details:not(.vm-panel)');
    }

    function ensureInjected() {
        const list = tabList();
        if (!list) {
            if (active) setActive(false);
            return;
        }

        if (!tabButton) {
            tabButton = el('button', {type: 'button', role: 'tab', class: 'vm-tab-button', 'aria-selected': 'false', tabindex: -1}, t('tab'));
            tabButton.addEventListener('click', () => setActive(true));
        }

        // Sits just before the last tab (System), whatever tabs the device has.
        const natives = nativeButtons(list);
        const anchor = natives[natives.length - 1] ?? null;
        if (tabButton.parentElement !== list || tabButton.nextElementSibling !== anchor) {
            list.insertBefore(tabButton, anchor);
        }

        if (!list.dataset.vmHooked) {
            list.dataset.vmHooked = '1';
            list.addEventListener('click', (e) => {
                const target = e.target.closest('button');
                if (target && target !== tabButton && target.parentElement === list) setActive(false, target);
            }, true);
        }

        // The UI switched tabs by itself (eg. a click on the device picture)
        if (active && list.querySelector(':scope > button.active:not(.vm-tab-button)')) {
            setActive(false);
        }

        if (!panel || panel.parentElement !== list.parentElement) {
            panel = el('div', {class: 'tabs-details vm-panel', role: 'tabpanel', 'aria-label': 'Voicemeeter'});
            panel.style.display = 'none';
            list.parentElement.append(panel);
            if (active) {
                active = false;
                setActive(true);
            }
        }
    }

    function setActive(state, clicked = null) {
        const list = tabList();
        if (state === active || !list) {
            active = state && !!list;
            return;
        }
        active = state;
        const details = nativeDetails(list);

        if (active) {
            restoreButton = list.querySelector(':scope > button.active:not(.vm-tab-button)');
            nativeButtons(list).forEach((b) => b.classList.remove('active'));
            tabButton.classList.add('active');
            tabButton.setAttribute('aria-selected', 'true');
            if (details) details.style.display = 'none';
            panel.style.display = '';

            loadLocale().then(refresh);
            clearInterval(pollTimer);
            pollTimer = setInterval(refresh, POLL_ACTIVE);
        } else {
            clearInterval(pollTimer);
            pollTimer = null;
            flush();

            tabButton.classList.remove('active');
            tabButton.setAttribute('aria-selected', 'false');
            if (details) details.style.display = '';
            if (panel) panel.style.display = 'none';

            // If Vue thinks the clicked tab was already active it won't re-render, put the class back.
            const restore = clicked ?? restoreButton;
            setTimeout(() => {
                if (!list.querySelector(':scope > button.active') && restore) restore.classList.add('active');
            }, 0);
        }
    }

    // Ctrl+Shift+<n> switches tabs in the main UI
    window.addEventListener('keydown', (e) => {
        if (!active || !e.ctrlKey || !e.shiftKey || !/^Digit[1-8]$/.test(e.code)) return;
        const list = tabList();
        if (list) setActive(false, nativeButtons(list)[Number(e.code[5]) - 1]);
    });

    let scheduled = false;
    const observer = new MutationObserver(() => {
        if (scheduled) return;
        scheduled = true;
        // Not requestAnimationFrame, that doesn't fire while the window is hidden
        setTimeout(() => {
            scheduled = false;
            ensureInjected();
        });
    });

    function start() {
        observer.observe(document.body, {childList: true, subtree: true});
        loadLocale();
        ensureInjected();
    }

    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start);
    else start();
})();
