// ES OPS - Diğer Sosyal Medya Platformları Bağlantı Akışı Testleri (Node.js).
//
// v1.0 kararı: platform uygulama kimlikleri EXE'ye derleme anında gömülür.
// Ayarlar ekranında HİÇBİR teknik kimlik formu görünmez ve JavaScript
// tarafında config ön-kontrolü YAPILMAZ. Her platformun "Bağlan" düğmesi
// doğrudan `x_connect` / `tiktok_connect` / `linkedin_connect` /
// `linkedin_connect` / `youtube_connect` komutunu çağırır; kimlik eksikse
// Rust kontrollü hata koduyla döner ve tek, kullanıcı dostu bildirim
// üretilir (sahte bağlantı / sahte başarı üretilmez).
//
// Bu testler şunları doğrular:
//   - Ayarlar listesinde hiçbir teknik kimlik formunun üretilmediği.
//   - OAuth öncesi kullanıcı adı alanının hiçbir platformda üretilmediği.
//   - Kimlikler eksikken bağlantı komutunun yine de çağrıldığı ve tek,
//     kullanıcı dostu hata bildirimi üretildiği (config ön-kontrolü yok).
//   - `*_config_status` komutunun hiç çağrılmadığı.
//   - Kimlikler hazırken bağlantı komutunun tek kez ve boş argümanla
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

test('Ayarlar listesinde HICBIR teknik kimlik formu uretilmiyor (TikTok dahil)', async () => {
    const sandbox = loadApp(() => null);

    sandbox.ayarlarPlatformListele();
    const html = sandbox.document.getElementById('ayarlarPlatformListesi').innerHTML;

    const yasakli = [
        'ayarlarTiktokConfigGrubu',
        'ayarlarTiktokClientKey',
        'ayarlarTiktokClientSecret',
        'ayarlarTiktokConfigDurum',
        'ayarlarXConfigGrubu',
        'ayarlarXClientKey',
        'ayarlarXConsumerSecret',
        'ayarlarMetaConfigGrubu',
        'ayarlarMetaAppId',
        'ayarlarMetaAppSecret',
        'ayarlarLinkedinConfigGrubu',
        'ayarlarLinkedinClientId',
    ];
    for (const alan of yasakli) {
        if (html.indexOf(alan) !== -1) {
            throw new Error('Teknik kimlik alani gereksiz yere uretildi: ' + alan);
        }
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

test('TikTok: config_status cagrilmaz, kimlik eksikken tiktok_connect yine de cagrilir ve tek hata bildirimi olur', async () => {
    const cagrilar = [];
    const sandbox = loadApp((cmd) => {
        cagrilar.push(cmd);
        if (cmd === 'tiktok_connect') {
            return Promise.reject({ code: 'tiktok_not_configured' });
        }
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    sandbox.ayarlarPlatformBaglan('tiktok');
    await new Promise((r) => setTimeout(r, 0));

    if (cagrilar.indexOf('tiktok_config_status') !== -1) {
        throw new Error('Config on-kontrolu kaldirildi: tiktok_config_status cagrilmamali');
    }
    if (cagrilar.indexOf('tiktok_connect') === -1) {
        throw new Error('Kimlik eksik olsa bile tiktok_connect cagrilmali');
    }
    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Beklenen 1 bildirim, olusan: ' + (sandbox.bildirimler.length - baslangic));
    }
    const b = sandbox.bildirimler[sandbox.bildirimler.length - 1];
    if (b.tur !== 'hata') {
        throw new Error('Hata bildirimi beklenir');
    }
    if (b.aciklama.indexOf('yapilandirilmamis') === -1 && b.aciklama.indexOf('kimlikleri') === -1) {
        throw new Error('Kullanici dostu mesaj bekleniyor: ' + b.aciklama);
    }
});

test('TikTok: kimlikler hazirken tiktok_connect tek kez ve bos argumanla cagrilir', async () => {
    const gidenArgs = [];
    const cagrilar = [];
    const sandbox = loadApp((cmd, args) => {
        cagrilar.push(cmd);
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

    if (cagrilar.indexOf('tiktok_config_status') !== -1) {
        throw new Error('Config on-kontrolu kaldirildi: tiktok_config_status cagrilmamali');
    }
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

test('X: config_status cagrilmaz, kimlik eksikken x_connect yine de cagrilir ve tek hata bildirimi olur', async () => {
    const cagrilar = [];
    const sandbox = loadApp((cmd) => {
        cagrilar.push(cmd);
        if (cmd === 'x_connect') {
            return Promise.reject({ code: 'x_not_configured' });
        }
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    sandbox.ayarlarPlatformBaglan('x');
    await new Promise((r) => setTimeout(r, 0));

    if (cagrilar.indexOf('x_config_status') !== -1) {
        throw new Error('Config on-kontrolu kaldirildi: x_config_status cagrilmamali');
    }
    if (cagrilar.indexOf('x_connect') === -1) {
        throw new Error('Kimlik eksik olsa bile x_connect cagrilmali');
    }
    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Beklenen 1 bildirim, olusan: ' + (sandbox.bildirimler.length - baslangic));
    }
    const b = sandbox.bildirimler[sandbox.bildirimler.length - 1];
    if (b.tur !== 'hata') {
        throw new Error('Hata bildirimi beklenir');
    }
    if (b.aciklama.indexOf('yapilandirilmamis') === -1 && b.aciklama.indexOf('kimlikleri') === -1) {
        throw new Error('Kullanici dostu mesaj bekleniyor: ' + b.aciklama);
    }
});

test('X: kimlikler hazirken x_connect tek kez ve bos argumanla cagrilir', async () => {
    const gidenArgs = [];
    const cagrilar = [];
    const sandbox = loadApp((cmd, args) => {
        cagrilar.push(cmd);
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

    if (cagrilar.indexOf('x_config_status') !== -1) {
        throw new Error('Config on-kontrolu kaldirildi: x_config_status cagrilmamali');
    }
    if (gidenArgs.length !== 1) {
        throw new Error('x_connect tam 1 kez cagrilmali');
    }
    if (JSON.stringify(gidenArgs[0]) !== '{}') {
        throw new Error('Bos arguman beklenir: ' + JSON.stringify(gidenArgs[0]));
    }
});

test('LinkedIn: config_status cagrilmaz, kimlik eksikken linkedin_connect yine de cagrilir ve tek hata bildirimi olur', async () => {
    const cagrilar = [];
    const sandbox = loadApp((cmd) => {
        cagrilar.push(cmd);
        if (cmd === 'linkedin_connect') {
            return Promise.reject({ code: 'linkedin_not_configured' });
        }
        return null;
    });

    const baslangic = sandbox.bildirimler.length;
    sandbox.ayarlarPlatformBaglan('linkedin');
    await new Promise((r) => setTimeout(r, 0));

    if (cagrilar.indexOf('linkedin_config_status') !== -1) {
        throw new Error('Config on-kontrolu kaldirildi: linkedin_config_status cagrilmamali');
    }
    if (cagrilar.indexOf('linkedin_connect') === -1) {
        throw new Error('Kimlik eksik olsa bile linkedin_connect cagrilmali');
    }
    if (sandbox.bildirimler.length !== baslangic + 1) {
        throw new Error('Beklenen 1 bildirim, olusan: ' + (sandbox.bildirimler.length - baslangic));
    }
    const b = sandbox.bildirimler[sandbox.bildirimler.length - 1];
    if (b.tur !== 'hata') {
        throw new Error('Hata bildirimi beklenir');
    }
    if (b.aciklama.indexOf('yapilandirilmamis') === -1 && b.aciklama.indexOf('kimligi') === -1) {
        throw new Error('Kullanici dostu mesaj bekleniyor: ' + b.aciklama);
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
