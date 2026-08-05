// ES OPS - Sosyal Medya Bağlantı Akışı Testleri (Node.js, DOM taklitli).
//
// Bu testler app.js'i vm bağlamında yükler ve Instagram/Facebook bağlantı
// akışını, Meta yapılandırma durumlarına göre DENETLER:
//   - Tek tıklamada yalnızca TEK bildirim üretilir.
//   - Devam eden akış varken yeni tıklama yok sayılır (çift tıklama koruması).
//   - Aynı yapılandırma hatası art arda tekrar kaydedilmez (10 sn penceresi).
//   - Meta App ID mevcut / eksik durumlarında doğru tek bildirim üretilir.
//   - Kullanıcı adı bağlantıya gönderilmez (sahte bağlantı üretilmez).
//   - Bağlı olmayan hesapta "Bağlantıyı Kes" komutu çağrılmaz.
//   - OAuth başarısında gerçek hesap adı görüntülenir.
//
// Çalıştırma: node tests/social_meta_flow.test.js

'use strict';

const fs = require('fs');
const path = require('path');
const vm = require('vm');

const ROOT = path.resolve(__dirname, '..');
const APP_JS = path.join(ROOT, 'app.js');

// ---- DOM taklitleri ----

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

// ---- Test çalıştırıcı ----

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

// ---- Testler ----

test('Instagram: Meta yapilandirmasi eksikken tek ve dogru bildirim', async () => {
    const calls = [];
    const sandbox = loadApp((cmd) => {
        calls.push(cmd);
        if (cmd === 'instagram_connect') {
            return Promise.reject({ code: 'meta_not_configured' });
        }
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    sandbox.ayarlarPlatformBaglan('instagram');
    await new Promise((r) => setTimeout(r, 0));

    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Beklenen 1 bildirim, olusan: ' + (sandbox.bildirimler.length - baslangic));
    }
    const b = sandbox.bildirimler[sandbox.bildirimler.length - 1];
    if (b.baslik !== 'Instagram bağlantısı başlatılamadı') {
        throw new Error('Beklenen baslik bulunamadi: ' + b.baslik);
    }
    if (b.aciklama.indexOf('Meta uygulama kimligi (App ID) yapilandirilmamis') === -1) {
        throw new Error('Kullanici dostu mesaj bekleniyor: ' + b.aciklama);
    }
    if (calls.filter((c) => c === 'instagram_connect').length !== 1) {
        throw new Error('instagram_connect tam 1 kez cagrilmali');
    }
    if (calls.indexOf('meta_config_status') !== -1) {
        throw new Error('Eski on-kontrol komutu (meta_config_status) artik cagrilmamali');
    }
});

test('Facebook: App Secret eksikken tek bildirim (ortak Meta ayari)', async () => {
    const sandbox = loadApp((cmd) => {
        if (cmd === 'facebook_connect') {
            return Promise.reject({ code: 'app_secret_required' });
        }
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    sandbox.ayarlarPlatformBaglan('facebook');
    await new Promise((r) => setTimeout(r, 0));

    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Beklenen 1 bildirim, olusan: ' + (sandbox.bildirimler.length - baslangic));
    }
    const b = sandbox.bildirimler[sandbox.bildirimler.length - 1];
    if (b.baslik !== 'Facebook bağlantısı başlatılamadı') {
        throw new Error('Beklenen baslik bulunamadi: ' + b.baslik);
    }
});

test('Cift tiklamada ikinci bildirim uretilmez (devam eden akis)', async () => {
    let resolveFirst;
    const sandbox = loadApp((cmd) => {
        if (cmd === 'instagram_connect') {
            return new Promise((res) => { resolveFirst = res; });
        }
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    sandbox.ayarlarPlatformBaglan('instagram'); // akış başladı, henüz bitmedi
    sandbox.ayarlarPlatformBaglan('instagram'); // ikinci tıklama yok sayılmalı
    await new Promise((r) => setTimeout(r, 0));

    if (sandbox.bildirimler.length !== baslangic) {
        throw new Error('Akis surerken bildirim olusmamali');
    }

    // Akış başarıyla bitsin: yalnız başarı bildirimi eklenir.
    resolveFirst({
        connection: {
            connectionStatus: 'connected',
            accountDisplayName: 'gercekhesap',
        },
    });
    await new Promise((r) => setTimeout(r, 0));

    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Basari sonrasi 1 bildirim beklenir, olusan: ' + (sandbox.bildirimler.length - baslangic));
    }
    const b = sandbox.bildirimler[sandbox.bildirimler.length - 1];
    if (b.tur !== 'basarili') {
        throw new Error('Basarili bildirim beklenir');
    }
    const plat = sandbox.ayarlarPlatformBul('instagram');
    if (!plat.bagli || plat.hesapAdi !== 'gercekhesap') {
        throw new Error('Gerçek hesap adi görüntülenmemeli: ' + plat.hesapAdi);
    }
});

test('Ayni yapilandirma hatasi 10 saniye icinde tekrar kaydedilmez', async () => {
    const sandbox = loadApp((cmd) => {
        if (cmd === 'instagram_connect') {
            return Promise.reject({ code: 'meta_not_configured' });
        }
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    sandbox.ayarlarPlatformBaglan('instagram');
    await new Promise((r) => setTimeout(r, 0));
    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Ilk tiklamada 1 bildirim beklenir');
    }

    // Aynı hata kısa süre içinde tekrarlanır: bildirim çoğaltılmaz.
    sandbox.ayarlarPlatformBaglan('instagram');
    await new Promise((r) => setTimeout(r, 0));
    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Ayni hata tekrar kaydedilmemeli, olusan: ' + (sandbox.bildirimler.length - baslangic));
    }
});

test('Kullanici adi baglanti komutuna gonderilmez (saghte baglanti yok)', async () => {
    const gidenArgs = [];
    const sandbox = loadApp((cmd, args) => {
        if (cmd === 'instagram_connect') {
            gidenArgs.push(args);
            return Promise.reject({ code: 'meta_not_configured' });
        }
        return null;
    });

    sandbox.ayarlarPlatformBaglan('instagram');
    await new Promise((r) => setTimeout(r, 0));

    if (gidenArgs.length !== 1) {
        throw new Error('instagram_connect bir kez cagrilmali');
    }
    const keys = Object.keys(gidenArgs[0] || {});
    if (keys.some((k) => k.toLowerCase().indexOf('username') !== -1 || k.toLowerCase().indexOf('kullanici') !== -1 || k.toLowerCase().indexOf('hesap') !== -1)) {
        throw new Error('Baglanti komutuna kullanici adi gonderilmemeli: ' + JSON.stringify(gidenArgs[0]));
    }
    // Kullanıcı adı alanı değeri ne olursa olsun akış değişmemeli:
    // ayarlarPlatformlar kayıtlarında hesapAdi alanı bağlantıya hiç iletilmez.
    if (JSON.stringify(gidenArgs[0]) !== '{}') {
        throw new Error('Bos argüman beklenir: ' + JSON.stringify(gidenArgs[0]));
    }
});

test('Bagli olmayan hesapta "Baglantiyi Kes" komutu cagrilmaz', async () => {
    const cagrilar = [];
    const sandbox = loadApp((cmd) => {
        cagrilar.push(cmd);
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    const plat = sandbox.ayarlarPlatformBul('instagram');
    plat.bagli = false;

    sandbox.ayarlarPlatformKes('instagram');
    await new Promise((r) => setTimeout(r, 0));

    if (cagrilar.indexOf('social_disconnect_account') !== -1) {
        throw new Error('Bagli olmayan hesapta kesme komutu cagrilmamali');
    }
    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Tek bilgilendirme bildirimi beklenir');
    }
    const b = sandbox.bildirimler[sandbox.bildirimler.length - 1];
    if (b.baslik !== 'Instagram hesabı bağlı değil') {
        throw new Error('Beklenen baslik: ' + b.baslik);
    }
});

runAll();
