// ES OPS - Diğer Sosyal Medya Platformları Bağlantı Akışı Testleri (Node.js).
//
// Bu testler, Instagram/Meta düzeltmesiyle aynı denetim kalıplarını diğer
// platformlara (X, TikTok, LinkedIn, Pinterest, YouTube) uygular:
//   - Config formlarının ayarlar listesinde render edildiği (TikTok dahil).
//   - OAuth öncesi kullanıcı adı alanının hiçbir platformda üretilmediği.
//   - Eksik yapılandırmada tek, kullanıcı dostu bildirim ve bağlantı komutunun
//     çağrılmadığı.
//   - Yapılandırma hazırken bağlantı komutunun tek kez ve boş argümanla
//     çağrıldığı.
//
// Çalıştırma: node tests/social_other_platforms.test.js

'use strict';

const fs = require('fs');
const path = require('path');
const vm = require('vm');

const ROOT = path.resolve(__dirname, '..');
const APP_JS = path.join(ROOT, 'app.js');

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

test('TikTok config formu ayarlar listesinde render ediliyor', async () => {
    const sandbox = loadApp(() => null);

    sandbox.ayarlarPlatformListele();
    const html = sandbox.document.getElementById('ayarlarPlatformListesi').innerHTML;

    if (html.indexOf('ayarlarTiktokConfigGrubu') === -1) {
        throw new Error('ayarlarTiktokConfigGrubu render edilmemis');
    }
    if (html.indexOf('ayarlarTiktokClientKey') === -1) {
        throw new Error('ayarlarTiktokClientKey alani yok');
    }
    if (html.indexOf('ayarlarTiktokClientSecret') === -1) {
        throw new Error('ayarlarTiktokClientSecret alani yok');
    }
    if (html.indexOf('ayarlarTiktokConfigDurum') === -1) {
        throw new Error('ayarlarTiktokConfigDurum gosterge alani yok');
    }
});

test('OAuth oncesi kullanici adi alani HICBIR platformda uretilmiyor', async () => {
    const sandbox = loadApp(() => null);

    sandbox.ayarlarPlatformListele();
    const html = sandbox.document.getElementById('ayarlarPlatformListesi').innerHTML;

    if (html.indexOf('ayarlarHesapAdi_') !== -1) {
        throw new Error('Kullanici adi alani gereksiz yere uretildi');
    }
});

test('TikTok: Client Key/Secret eksikken tek bildirim, baglanti komutu cagrilmaz', async () => {
    const cagrilar = [];
    const sandbox = loadApp((cmd) => {
        cagrilar.push(cmd);
        if (cmd === 'tiktok_config_status') {
            return Promise.resolve({ clientKeyConfigured: false, clientSecretConfigured: false });
        }
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    sandbox.ayarlarPlatformBaglan('tiktok');
    await new Promise((r) => setTimeout(r, 0));

    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Beklenen 1 bildirim, olusan: ' + (sandbox.bildirimler.length - baslangic));
    }
    const b = sandbox.bildirimler[sandbox.bildirimler.length - 1];
    if (b.baslik !== 'TikTok icin Client Key / Client Secret gerekli') {
        throw new Error('Beklenen baslik bulunamadi: ' + b.baslik);
    }
    if (cagrilar.indexOf('tiktok_connect') !== -1) {
        throw new Error('Eksik yapilandirmada tiktok_connect cagrilmamali');
    }
});

test('TikTok: kimlikler hazirken tiktok_connect tek kez ve bos argumanla cagrilir', async () => {
    const gidenArgs = [];
    const sandbox = loadApp((cmd, args) => {
        if (cmd === 'tiktok_config_status') {
            return Promise.resolve({ clientKeyConfigured: true, clientSecretConfigured: true });
        }
        if (cmd === 'tiktok_connect') {
            gidenArgs.push(args);
            return Promise.resolve({
                connection: { connectionStatus: 'connected', accountDisplayName: 'tiktoker' },
            });
        }
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    sandbox.ayarlarPlatformBaglan('tiktok');
    await new Promise((r) => setTimeout(r, 0));

    if (gidenArgs.length !== 1) {
        throw new Error('tiktok_connect tam 1 kez cagrilmali');
    }
    if (JSON.stringify(gidenArgs[0]) !== '{}') {
        throw new Error('Bos arguman beklenir: ' + JSON.stringify(gidenArgs[0]));
    }
    const plat = sandbox.ayarlarPlatformBul('tiktok');
    if (!plat.bagli || plat.hesapAdi !== 'tiktoker') {
        throw new Error('Gerçek hesap adi yuklenmeli');
    }
    const basari = sandbox.bildirimler[sandbox.bildirimler.length - 1];
    if (basari.tur !== 'basarili') {
        throw new Error('Basarili bildirim beklenir');
    }
});

test('X: Consumer Key/Secret eksikken tek bildirim, x_connect cagrilmaz', async () => {
    const cagrilar = [];
    const sandbox = loadApp((cmd) => {
        cagrilar.push(cmd);
        if (cmd === 'x_config_status') {
            return Promise.resolve({ consumerKeyConfigured: false, consumerSecretConfigured: false });
        }
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    sandbox.ayarlarPlatformBaglan('x');
    await new Promise((r) => setTimeout(r, 0));

    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Beklenen 1 bildirim, olusan: ' + (sandbox.bildirimler.length - baslangic));
    }
    if (cagrilar.indexOf('x_connect') !== -1) {
        throw new Error('Eksik yapilandirmada x_connect cagrilmamali');
    }
});

test('X: kimlikler hazirken x_connect tek kez cagrilir', async () => {
    const gidenArgs = [];
    const sandbox = loadApp((cmd, args) => {
        if (cmd === 'x_config_status') {
            return Promise.resolve({ consumerKeyConfigured: true, consumerSecretConfigured: true });
        }
        if (cmd === 'x_connect') {
            gidenArgs.push(args);
            return Promise.resolve({
                connection: { connectionStatus: 'connected', accountDisplayName: 'X @test' },
            });
        }
        return null;
    });

    sandbox.ayarlarPlatformBaglan('x');
    await new Promise((r) => setTimeout(r, 0));

    if (gidenArgs.length !== 1) {
        throw new Error('x_connect tam 1 kez cagrilmali');
    }
    if (JSON.stringify(gidenArgs[0]) !== '{}') {
        throw new Error('Bos arguman beklenir: ' + JSON.stringify(gidenArgs[0]));
    }
});

test('LinkedIn: Client ID eksikken tek bildirim, linkedin_connect cagrilmaz', async () => {
    const cagrilar = [];
    const sandbox = loadApp((cmd) => {
        cagrilar.push(cmd);
        if (cmd === 'linkedin_config_status') {
            return Promise.resolve({ clientIdConfigured: false });
        }
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    sandbox.ayarlarPlatformBaglan('linkedin');
    await new Promise((r) => setTimeout(r, 0));

    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Beklenen 1 bildirim, olusan: ' + (sandbox.bildirimler.length - baslangic));
    }
    if (cagrilar.indexOf('linkedin_connect') !== -1) {
        throw new Error('Eksik yapilandirmada linkedin_connect cagrilmamali');
    }
});

test('Pinterest: Client ID/Secret eksikken tek bildirim, pinterest_connect cagrilmaz', async () => {
    const cagrilar = [];
    const sandbox = loadApp((cmd) => {
        cagrilar.push(cmd);
        if (cmd === 'pinterest_config_status') {
            return Promise.resolve({ clientIdConfigured: false, clientSecretConfigured: false });
        }
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    sandbox.ayarlarPlatformBaglan('pinterest');
    await new Promise((r) => setTimeout(r, 0));

    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Beklenen 1 bildirim, olusan: ' + (sandbox.bildirimler.length - baslangic));
    }
    if (cagrilar.indexOf('pinterest_connect') !== -1) {
        throw new Error('Eksik yapilandirmada pinterest_connect cagrilmamali');
    }
});

test('YouTube: client id yokken tek hata bildirimi, sahte baglanti yok', async () => {
    const sandbox = loadApp((cmd) => {
        if (cmd === 'youtube_connect') {
            return Promise.reject({ code: 'youtube_not_configured' });
        }
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    sandbox.ayarlarPlatformBaglan('youtube');
    await new Promise((r) => setTimeout(r, 0));

    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Beklenen 1 bildirim, olusan: ' + (sandbox.bildirimler.length - baslangic));
    }
    const b = sandbox.bildirimler[sandbox.bildirimler.length - 1];
    if (b.tur !== 'hata') {
        throw new Error('Hata bildirimi beklenir');
    }
    if (b.aciklama.indexOf('yapilandirilmamis') === -1 && b.aciklama.indexOf('tanimli degil') === -1) {
        throw new Error('Kullanici dostu mesaj bekleniyor: ' + b.aciklama);
    }
    const plat = sandbox.ayarlarPlatformBul('youtube');
    if (plat.bagli) {
        throw new Error('Client id yokken sahte baglanti gosterilmemeli');
    }
});

runAll();
