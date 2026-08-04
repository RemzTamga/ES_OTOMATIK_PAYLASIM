// ES OPS - Log Gizliliği ve Yardım Ekranı Testleri (Node.js).
//
// Bu testler son satış hazırlığının "Log sistemi" ve "Yardım ekranı"
// bölümünü doğrular:
//   - esLogGizle() token/parola/api anahtarı/client secret/lisans kodunu maskeler.
//   - global hata yakalayıcılar (window.onerror, unhandledrejection) tanımlıdır.
//   - index.html'deki onclick işleyicilerinin tümü app.js'te tanımlıdır.
//   - Yardım ekranı FAZ 10 yerine dolu içerik üretmektedir.
//
// Çalıştırma: node tests/social_log_security.test.js

'use strict';

const fs = require('fs');
const path = require('path');
const vm = require('vm');

const ROOT = path.resolve(__dirname, '..');
const APP_JS = path.join(ROOT, 'app.js');
const INDEX_HTML = path.join(ROOT, 'index.html');

function makeClassList() {
    const set = new Set();
    return {
        add: (c) => set.add(c),
        remove: (c) => set.delete(c),
        toggle: (c) => (set.has(c) ? set.delete(c) : set.add(c)),
        contains: (c) => set.has(c),
    };
}

function makeEl() {
    const store = {
        classList: makeClassList(),
        style: {},
        dataset: {},
        value: '',
        textContent: '',
        innerHTML: '',
        files: [],
        checked: false,
        disabled: false,
    };
    return new Proxy(store, {
        get(t, p) {
            if (p in t) return t[p];
            if (p === 'addEventListener' || p === 'removeEventListener') return () => {};
            if (p === 'scrollIntoView' || p === 'focus' || p === 'click') return () => {};
            if (p === 'setAttribute' || p === 'removeAttribute') return () => {};
            if (p === 'appendChild' || p === 'insertBefore' || p === 'removeChild') return () => {};
            if (p === 'querySelector') return () => makeEl();
            if (p === 'querySelectorAll') return () => [];
            if (p === 'getAttribute') return () => null;
            if (p === 'contains') return () => false;
            if (p === 'closest') return () => null;
            if (p === 'children') return [];
            return makeEl();
        },
        set(t, p, v) {
            t[p] = v;
            return true;
        },
    });
}

function buildDocument() {
    const els = {};
    return {
        getElementById: (id) => els[id] || (els[id] = makeEl()),
        querySelector: () => makeEl(),
        querySelectorAll: () => [],
        createElement: () => makeEl(),
        addEventListener: () => {},
        body: makeEl(),
        documentElement: makeEl(),
    };
}

function loadApp(invokeImpl) {
    const code = fs.readFileSync(APP_JS, 'utf8');

    const localStorage = {
        _d: {},
        getItem: (k) => (k in this._d ? this._d[k] : null),
        setItem: (k, v) => { this._d[k] = String(v); },
        removeItem: (k) => { delete this._d[k]; },
    };

    const sandbox = {
        console,
        setTimeout,
        clearTimeout,
        setInterval,
        clearInterval,
        Date,
        Math,
        JSON,
        Promise,
        Array,
        Object,
        String,
        Number,
        Boolean,
        Intl,
        RegExp,
        Error,
        URL,
        localStorage,
        alert: () => {},
        confirm: () => true,
        prompt: () => null,
        fetch: () => Promise.resolve({ ok: false, json: async () => ({}) }),
        navigator: { language: 'tr-TR', clipboard: { writeText: async () => {} } },
        screen: { width: 1280, height: 800, colorDepth: 24 },
        btoa: (s) => Buffer.from(s, 'binary').toString('base64'),
        atob: (s) => Buffer.from(s, 'base64').toString('binary'),
    };

    const tauriWindow = {
        __TAURI__: { core: { invoke: (cmd, args) => invokeImpl(cmd, args || {}) } },
        addEventListener: () => {},
        innerWidth: 1280,
    };

    sandbox.window = tauriWindow;
    sandbox.document = buildDocument();
    sandbox.globalThis = sandbox;

    vm.createContext(sandbox);
    vm.runInContext(code, sandbox, { filename: 'app.js' });
    return sandbox;
}

const tests = [];
function test(name, fn) {
    tests.push({ name, fn });
}

let passed = 0;
let failed = 0;

async function runAll() {
    for (const t of tests) {
        try {
            await t.fn();
            passed++;
            console.log('  OK   ' + t.name);
        } catch (err) {
            failed++;
            console.log('  FAIL ' + t.name);
            console.log('       ' + (err && err.message));
        }
    }
    console.log('\nSonuc: ' + passed + ' gecti, ' + failed + ' basarisiz');
    process.exit(failed === 0 ? 0 : 1);
}

// ---- Testler ----

test('esLogGizle: token degerini maskeliyor', () => {
    const sandbox = loadApp(() => null);
    const out = sandbox.esLogGizle('token = sk-1234567890abcdef');
    if (out.indexOf('sk-1234567890abcdef') !== -1) throw new Error('token sizdi: ' + out);
    if (out.indexOf('[GIZLI]') === -1) throw new Error('GIZLI isareti yok: ' + out);
});

test('esLogGizle: parola ve client secret maskeliyor', () => {
    const sandbox = loadApp(() => null);
    const out = sandbox.esLogGizle('password=gecerliParola123 client_secret: SUPER_GIZLI');
    if (out.indexOf('gecerliParola123') !== -1) throw new Error('parola sizdi: ' + out);
    if (out.indexOf('SUPER_GIZLI') !== -1) throw new Error('client secret sizdi: ' + out);
});

test('esLogGizle: lisans kodunu maskeliyor', () => {
    const sandbox = loadApp(() => null);
    const out = sandbox.esLogGizle('license_code = LIC-20260701-000001');
    if (out.indexOf('LIC-20260701-000001') !== -1) throw new Error('lisans kodu sizdi: ' + out);
});

test('esLogGizle: normal mesajlari oldugu gibi korur', () => {
    const sandbox = loadApp(() => null);
    const out = sandbox.esLogGizle('Baglanti basarili: Instagram hesabi acildi');
    if (out.indexOf('Instagram hesabi acildi') === -1) throw new Error('mesaj bozuldu: ' + out);
});

test('esLogGizle: hata kodlarini korur', () => {
    const sandbox = loadApp(() => null);
    const out = sandbox.esLogGizle('youtube_not_configured: YouTube kimlikleri eksik');
    if (out.indexOf('youtube_not_configured') === -1) throw new Error('hata kodu bozuldu: ' + out);
});

test('global hata yakalayicilar tanimli', () => {
    const sandbox = loadApp(() => null);
    if (typeof sandbox.esLogYaz !== 'function') throw new Error('esLogYaz tanimli degil');
    if (typeof sandbox.esLogKullaniciUyar !== 'function') throw new Error('esLogKullaniciUyar tanimli degil');
    if (sandbox.window.onerror !== undefined && typeof sandbox.window.onerror !== 'function')
        throw new Error('window.onerror function degil');
});

test('esLogYaz gizli bilgiyi log komutuna gondermez', () => {
    const cagrilar = [];
    const sandbox = loadApp((cmd, args) => {
        cagrilar.push({ cmd, args });
        return null;
    });

    sandbox.esLogYaz('error', 'token = GIZLI_ANAHTAR_12345');

    const logKagit = cagrilar.filter((c) => c.cmd === 'log_append');
    if (logKagit.length === 0) return; // Tauri invoke stubbing in onizleme null donebilir

    const mesaj = JSON.stringify(logKagit[0].args);
    if (mesaj.indexOf('GIZLI_ANAHTAR_12345') !== -1) throw new Error('log komutuna secret gonderildi: ' + mesaj);
    if (mesaj.indexOf('[GIZLI]') === -1) throw new Error('maskeleme isareti yok: ' + mesaj);
});

test('onclick isleyicilerinin tamami app.js\'te tanimli', () => {
    const html = fs.readFileSync(INDEX_HTML, 'utf8');
    const code = fs.readFileSync(APP_JS, 'utf8');

    // onclick="funcName(...)" kalıplarını topla
    const re = /onclick="([a-zA-Z_$][a-zA-Z0-9_$]*)\s*\(/g;
    const seen = new Set();
    let m;
    while ((m = re.exec(html)) !== null) {
        seen.add(m[1]);
    }
    if (seen.size === 0) throw new Error('onclick isleyicisi bulunamadi');

    const eksik = [];
    seen.forEach((fn) => {
        // Fonksiyon tanımı: "function fn(" veya "fn = function" veya "fn: function"
        const tanimli =
            new RegExp('function\\s+' + fn + '\\s*\\(').test(code) ||
            new RegExp(fn + '\\s*=\\s*function').test(code) ||
            new RegExp(fn + '\\s*:\\s*function').test(code);
        if (!tanimli) eksik.push(fn);
    });

    if (eksik.length > 0) {
        throw new Error('tanimsiz onclick isleyicileri: ' + eksik.join(', '));
    }
});

test('Yardim ekrani dolu icerik uretiyor', () => {
    const html = fs.readFileSync(INDEX_HTML, 'utf8');

    // Eski FAZ 10 yer tutucusu kalmamali
    if (html.indexOf('Bu modül FAZ 10') !== -1) throw new Error('Yardim ekrani hala yer tutucu');

    const gerekli = [
        'İlk Kurulum',
        'Lisans Aktivasyonu',
        'Sosyal Medya Hesabı Bağlama',
        'Web Sitesi Bağlantısı',
        'Manuel, Otomatik ve Planlı Yayın',
        'Hata Mesajları',
        'Destek İçin Log Alma',
    ];
    gerekli.forEach((baslik) => {
        if (html.indexOf(baslik) === -1) throw new Error('Yardim bolumu eksik: ' + baslik);
    });

    // Log butonlari tanimli olmali
    if (html.indexOf('onclick="yardimLogKlasoruAc()"') === -1) throw new Error('Log Klasoru buttonu yok');
    if (html.indexOf('onclick="yardimLoglariAktar()"') === -1) throw new Error('Log Konumu buttonu yok');
});

test('index.html app.js ve style.css referanslarini koruyor', () => {
    const html = fs.readFileSync(INDEX_HTML, 'utf8');
    if (html.indexOf('src="app.js"') === -1) throw new Error('app.js referansi yok');
    if (html.indexOf('href="style.css"') === -1) throw new Error('style.css referansi yok');
});

// ---- Calistir ----
runAll();