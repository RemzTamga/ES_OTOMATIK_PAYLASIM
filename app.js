// ===== GLOBAL HATA YAKALAMA VE LOG SISTEMI =====
// Beklenmeyen JavaScript hatalari sessizce gecistirilmez; uygulamanin veri
// klasorundeki log dosyasina yazilir ve kullaniciya kisaca bildirilir.
// Uygulama mumkunse calismaya devam eder; sahte basari uretilmez.
var esLogSonUyariZamani = 0;

// Hassas bilgileri loglara yazilmadan once maskeler:
// token, parola, API anahtari, Client Secret, lisans kodu.
function esLogGizle(text) {
    var s = String(text == null ? '' : text);

    // 1) Bilinen alan adlari + deger: token/secret/password/api_key/license vb.
    var hassasAnahtarlar = [
        'api_key', 'apikey', 'api key', 'client_secret', 'client secret',
        'consumer_secret', 'consumer key', 'access_token', 'refresh_token',
        'authorization', 'password', 'passwd', 'pwd', 'token', 'secret',
        'license_key', 'license_code', 'license code', 'lisans_kodu', 'license',
        'client_id', 'app_secret', 'app_id'
    ];
    var i;
    for (i = 0; i < hassasAnahtarlar.length; i++) {
        var key = hassasAnahtarlar[i];
        // key = deger | key: deger | "key":"deger" | key deger
        var re = new RegExp('(^|[^a-zA-Z0-9_])' + key.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
            + '\\s*(=|:)?\\s*("[^"]*"|\'[^\']*\'|[^\\s,;}]+)', 'gi');
        s = s.replace(re, function(match, pre, sep, val) {
            // Deger zaten gizli ise dokunma
            if (String(val).indexOf('[GIZLI]') !== -1) return match;
            return pre + key + (sep || '') + ' [GIZLI]';
        });
    }

    // 2) JWT / uzun token benzeri bloklar (20+ karakter ve gercek anahtar
    //    gorunumunde: buyuk harf, rakam, '-' veya '.' icerir).
    //    Yalnizca alt cizgili kucuk harfli diziler (ornegin youtube_not_configured
    //    gibi hata kodlari) maskelenmez.
    s = s.replace(/(?:[A-Za-z0-9_\-.]{20,})/g, function(blok) {
        if (/^[a-z_]+$/.test(blok)) return blok;
        return '[GIZLI]';
    });

    // 3) Bearer <token> kalibi
    s = s.replace(/([Bb]earer\s*=\s*|[Bb]earer\s+)[^\s,;}]+/g, '$1[GIZLI]');

    return s;
}

// Log kaydini uygulamanin log dosyasina yazar (Tauri ortaminda) ve
// gelistirici konsoluna da dusurur. Onizleme (Tauri disi) modunda yalniz
// konsola yazilir; hata uretilmez.
function esLogYaz(level, mesaj) {
    var temiz = esLogGizle(mesaj);
    try {
        if (level === 'error') {
            console.error('[ES OPS] ' + temiz);
        } else {
            console.log('[ES OPS] ' + temiz);
        }
    } catch (e) {}
    // Platform baglantilari icin kullanilan bilinmeyenlerin buraya
    // gonderilmesi hata uretmemeli; yalniz gercek Tauri ortaminda yazar.
    if (window.__TAURI__ && window.__TAURI__.core) {
        try {
            var lvl = level || 'INFO';
            var sonuc = window.__TAURI__.core.invoke('log_append', {
                level: lvl,
                message: temiz
            });
            if (sonuc && typeof sonuc.catch === 'function') {
                sonuc.catch(function() {});
            }
        } catch (e) {}
    }
}

// Kullaniciya kisa ve anlasilir bir hata bildirimi gosterir.
// Sik tekrar eden hatalarda sayfa eksi tonlari onlemek icin esik kullanilir.
function esLogKullaniciUyar(mesaj) {
    var simdi = Date.now();
    if (simdi - esLogSonUyariZamani < 8000) return;
    esLogSonUyariZamani = simdi;

    try {
        var onceki = document.getElementById('esOpsHataToast');
        if (onceki) onceki.remove();

        var toast = document.createElement('div');
        toast.id = 'esOpsHataToast';
        toast.style.cssText = 'position:fixed;top:14px;right:14px;z-index:99999;max-width:380px;background:#fff;border-radius:10px;box-shadow:0 8px 24px rgba(0,0,0,0.18);padding:14px 16px;font-size:0.85rem;border-left:4px solid #ef4444;';
        toast.innerHTML = '<div style="font-weight:700;color:#ef4444;margin-bottom:4px;">Beklenmeyen bir hata oluştu</div>' +
            '<div style="color:#4b5563;line-height:1.5;">Uygulama çalışmaya devam ediyor. Hata kaydı log dosyasına yazıldı. Detay için <strong>Yardım &rsaquo; Destek &rsaquo; Loglar</strong> bölümünü kullanın.</div>';
        document.body.appendChild(toast);
        setTimeout(function() {
            var el = document.getElementById('esOpsHataToast');
            if (el) el.remove();
        }, 9000);
    } catch (e) {}
}

// window.onerror: yakalanmamis istisnalar buraya duser.
window.onerror = function(mesaj, kaynak, satir, sutun, hata) {
    var detay = String(mesaj || 'Bilinmeyen hata');
    if (kaynak) detay += ' (' + kaynak + ':' + satir + ')';
    if (hata && hata.stack) detay = detay + '\n' + hata.stack;
    esLogYaz('error', detay);
    esLogKullaniciUyar(detay);
    // true donmek varsayilan davranisi (console'a dusme) engellemez; burada
    // hata uygulamaya yayilmaya devam eder, ancak loglanir.
    return true;
};

// Uzak yakalanmamis Promise hatalari (unhandledrejection) buraya duser.
window.addEventListener('unhandledrejection', function(e) {
    var nedeni = e && e.reason;
    var detay = nedeni && nedeni.stack ? nedeni.stack : (String(nedeni || 'Bilinmeyen promise hatasi'));
    esLogYaz('error', 'unhandledrejection: ' + detay);
    esLogKullaniciUyar(detay);
});

// ===== YARDIM EKRANI: LOG ISLEMLERI =====
// Log klasorunun adresini kullaniciya gosterir ve uygulama ici ortamda
// kopyalanmasina olanak tanir. Onizleme modunda yalniz adres bilgisi verilir.
function yardimLogKlasoruAc() {
    var p = esTauriInvoke('log_open_folder');
    if (!p) {
        alert('Log klasoru yalniz masaustu uygulamada acilabilir.');
        return;
    }
    p.then(function(yol) {
        alert('Log klasoru: ' + (yol || 'Belirtilmemiş'));
    }).catch(function(hata) {
        alert('Log klasoru acilamadi: ' + String(hata || 'bilinmeyen hata'));
    });
}

// Log dosyasini kullanici icin guncel tutmak amaciyla veri klasorundeki
// dosyanin guncel oldugunu bildirir. Daha sonra kullanici klasorunu acar.
// (Dosyanin disa aktarilmasi: gerekirse isletim sistemi kopyalama islemi
// ile kullanici kendi yapar; izin kisit olsa dahi yapay dosya uretilmez.)
function yardimLoglariAktar() {
    var p = esTauriInvoke('log_open_folder');
    if (!p) {
        alert('Loglar yalniz masaustu uygulamada dışa aktarilabilir.');
        return;
    }
    p.then(function(yol) {
        alert('Loglar kayitlidiniz. Lütfen bilgisayarınızda şu klasöre gidin ve ES OPS klasorunu acin:\n\n' +
            (yol || 'Belirtilmiş') + '\n\nBu klasordeki es-ops.log dosyalarini destek ekibine iletebilirsiniz.');
    }).catch(function(hata) {
        alert('Log adresi alinamadi: ' + String(hata || 'bilinmeyen hata'));
    });
}

// ===== SAYFA GECISLERI =====
function navigateTo(page) {
    document.querySelectorAll('.sidebar-menu a').forEach(function(el) {
        el.classList.remove('active');
        if (el.getAttribute('data-page') === page) {
            el.classList.add('active');
        }
    });

    var titles = {
        'dashboard': 'Dashboard',
        'paylasimlar': 'Paylasimlar',
        'medya': 'Medya Kutuphanesi',
        'yayin-gecmisi': 'Yayin Gecmisi',
        'raporlar': 'Raporlar',
        'bildirim': 'Bildirim Merkezi',
        'ayarlar': 'Ayarlar',
        'lisans': 'Lisans',
        'yardim': 'Yardim'
    };
    document.getElementById('pageTitle').textContent = titles[page] || 'Dashboard';

    document.querySelectorAll('.page-content').forEach(function(el) {
        el.classList.remove('active');
    });

    var targetPage = document.getElementById('page-' + page);
    if (targetPage) {
        targetPage.classList.add('active');
    }

    var tabBar = document.getElementById('tabBar');
    if (page === 'dashboard') {
        tabBar.style.display = 'flex';
    } else {
        tabBar.style.display = 'none';
    }

    closeMobileMenu();
}

// ===== DASHBOARD SEKMELERI =====
function switchDashboardTab(tab) {
    document.querySelectorAll('.tab-bar .tab').forEach(function(el) {
        el.classList.remove('active');
        if (el.getAttribute('data-tab') === tab) {
            el.classList.add('active');
        }
    });

    var smContent = document.getElementById('dash-sosyal-medya');
    var webContent = document.getElementById('dash-web-sitesi');

    if (tab === 'sosyal-medya') {
        smContent.style.display = 'block';
        webContent.style.display = 'none';
    } else {
        smContent.style.display = 'none';
        webContent.style.display = 'block';
    }
}

// ===== MOBIL MENU =====
function toggleMobileMenu() {
    var sidebar = document.getElementById('sidebar');
    var overlay = document.getElementById('sidebarOverlay');
    sidebar.classList.toggle('open');
    overlay.classList.toggle('show');
}

function closeMobileMenu() {
    var sidebar = document.getElementById('sidebar');
    var overlay = document.getElementById('sidebarOverlay');
    sidebar.classList.remove('open');
    overlay.classList.remove('show');
}

document.getElementById('sidebarOverlay').addEventListener('click', function() {
    closeMobileMenu();
});

// ===== ILK YUKLEME =====
navigateTo('dashboard');

// ===== FAZ 2 - PAYLASIM TAB DEGISIMI =====
function switchPaylasimTab(tab) {
    document.querySelectorAll('.paylasim-tabs .tab').forEach(function(el) {
        el.classList.remove('active');
        if (el.getAttribute('data-paylasim-tab') === tab) {
            el.classList.add('active');
        }
    });

    var tabs = ['standart', 'kampanya', 'detayli', 'duyuru'];
    tabs.forEach(function(t) {
        var el = document.getElementById('paylasim-' + t);
        if (el) {
            el.style.display = (t === tab) ? 'block' : 'none';
        }
    });
}

// ===== FAZ 2 - FILE UPLOAD YONETIMI =====
document.addEventListener('DOMContentLoaded', function() {
    setupFileUpload('standart');
    setupFileUpload('kampanya');
    setupFileUpload('detayli');
    setupFileUpload('duyuru');
});

// Gerçek disk yolları (native Tauri seçimi). Webview, kullanıcının seçtiği
// dosyanın gerçek yoluna erişemez; bu nedenle medya seçimi native dosya
// seçici ile yapılır ve mutlak yollar tutulur.
var medyaSecimYollari = {};

function medyaYolAdi(path) {
    var n = String(path || '').replace(/\\/g, '/');
    return n.substring(n.lastIndexOf('/') + 1);
}

function medyaVideoYolMu(path) {
    return /\.(mp4|mov|avi|mkv|webm|m4v|flv|3gp|mpeg|mpg|ogv|ts|wmv)$/i.test(path);
}

function medyaSecimPreview(prefix) {
    var container = document.getElementById(prefix + 'UploadedFiles');
    if (!container) return;
    var liste = medyaSecimYollari[prefix] || [];
    if (liste.length === 0) {
        container.innerHTML = '';
        return;
    }
    container.innerHTML = '';
    liste.forEach(function (ad, index) {
        var item = document.createElement('div');
        item.className = 'uploaded-file-item';
        item.innerHTML = '<span class="file-name">' + (index + 1) + '. ' + medyaYolAdi(ad) + '</span><span class="file-remove" onclick="medyaSecimKaldir(\'' + prefix + '\',' + index + ')">Kaldir</span>';
        container.appendChild(item);
    });
}

function medyaSecimKaldir(prefix, index) {
    if (!medyaSecimYollari[prefix]) return;
    medyaSecimYollari[prefix].splice(index, 1);
    medyaSecimPreview(prefix);
}

function medyaTauriSec(prefix) {
    var p = esTauriInvoke('pick_media_files');
    if (!p) return;
    var max = (prefix === 'detayli') ? 10 : 999;
    p.then(function (yollar) {
        if (!yollar || yollar.length === 0) return;
        var yeni = (medyaSecimYollari[prefix] || []).concat(yollar).slice(0, max);
        medyaSecimYollari[prefix] = yeni;
        medyaSecimPreview(prefix);
    }).catch(function () {});
}

// ---- Paylaşım formlarından gerçek kullanıcı verisini okuma ----
var formBaslikIds = {
    standart: 'standartAciklama',
    kampanya: 'kampanyaBaslik',
    detayli: 'detayliAciklama',
    duyuru: 'duyuruIcerik'
};
var formIcerikIds = {
    standart: 'standartAciklama',
    kampanya: 'kampanyaAciklama',
    detayli: 'detayliAciklama',
    duyuru: 'duyuruIcerik'
};
var formBaslikVarsayi = {
    standart: 'Standart Paylasim',
    kampanya: 'Kampanya Paylasimi',
    detayli: 'Detayli Paylasim',
    duyuru: 'Duyuru ve Ilanlar'
};

function formMetinOku(type, key) {
    var id = (key === 'baslik') ? formBaslikIds[type] : formIcerikIds[type];
    if (!id) return '';
    var el = document.getElementById(id);
    return el ? el.value.trim() : '';
}

function formBaslikAl(type) {
    return formMetinOku(type, 'baslik') || formBaslikVarsayi[type] || '';
}

function formIcerikAl(type) {
    return formMetinOku(type, 'icerik') || (formBaslikVarsayi[type] || '') + ' icerigi';
}

// Seçilen medyayı (öncelikle native gerçek yollar, yoksa tarayıcı dosya adları)
// toplar ve video yolunu/medya türünü türetir.
function secilenMedyaGetir(type) {
    var media = [];
    if (window.__TAURI__) {
        media = (medyaSecimYollari[type] || []).slice();
    }
    if (media.length === 0) {
        var inputEl = document.getElementById(type + 'FileInput');
        if (inputEl && inputEl.files && inputEl.files.length > 0) {
            media = Array.from(inputEl.files).map(function (f) { return f.name; });
        }
    }
    var videoPath = '';
    media.forEach(function (m) {
        if (!videoPath && medyaVideoYolMu(m)) videoPath = m;
    });
    var hasVideo = videoPath !== '';
    return {
        media: media,
        videoPath: videoPath,
        hasVideo: hasVideo,
        mediaKind: media.length ? (hasVideo ? 'video' : 'image') : ''
    };
}

function setupFileUpload(prefix) {
    var area = document.getElementById(prefix + 'UploadArea');
    var input = document.getElementById(prefix + 'FileInput');
    var container = document.getElementById(prefix + 'UploadedFiles');
    if (!area || !input || !container) return;

    area.addEventListener('click', function() {
        if (window.__TAURI__) {
            medyaTauriSec(prefix);
        } else {
            input.click();
        }
    });

    input.addEventListener('change', function() {
        var files = Array.from(this.files);
        var maxFiles = (prefix === 'detayli') ? 10 : 999;
        if (files.length > maxFiles) {
            files = files.slice(0, maxFiles);
        }
        container.innerHTML = '';
        files.forEach(function(file, index) {
            var item = document.createElement('div');
            item.className = 'uploaded-file-item';
            item.innerHTML = '<span class="file-name">' + (index + 1) + '. ' + file.name + '</span><span class="file-remove" onclick="this.parentElement.remove()">Kaldir</span>';
            container.appendChild(item);
        });
    });

    area.addEventListener('dragover', function(e) {
        e.preventDefault();
        area.style.borderColor = '#4f8cff';
        area.style.background = '#f0f4ff';
    });

    area.addEventListener('dragleave', function() {
        area.style.borderColor = '#d1d5db';
        area.style.background = '#f9fafb';
    });

    area.addEventListener('drop', function(e) {
        e.preventDefault();
        area.style.borderColor = '#d1d5db';
        area.style.background = '#f9fafb';
        var files = Array.from(e.dataTransfer.files);
        var maxFiles = (prefix === 'detayli') ? 10 : 999;
        if (files.length > maxFiles) {
            files = files.slice(0, maxFiles);
        }
        container.innerHTML = '';
        files.forEach(function(file, index) {
            var item = document.createElement('div');
            item.className = 'uploaded-file-item';
            item.innerHTML = '<span class="file-name">' + (index + 1) + '. ' + file.name + '</span><span class="file-remove" onclick="this.parentElement.remove()">Kaldir</span>';
            container.appendChild(item);
        });
        input.files = e.dataTransfer.files;
    });
}

// ===== FAZ 2 - SEMBOLIK KAYDET / SIMDI PAYLAS =====
// Kayit sirasinda secilen video dosyasinin gercek yolunu native dosya seçici
// ile çözer (yalniz video uzantili dosya icin). Görsel/metin icin boş döner.
function simulateSave(type) {
    var names = {
        'standart': 'Standart Paylasim',
        'kampanya': 'Kampanya Paylasimi',
        'detayli': 'Detayli Paylasim'
    };
    var siraNo = String(Math.floor(Math.random() * 899) + 100);

    var baslik = formBaslikAl(type);
    var mesaj = formIcerikAl(type);
    var secim = secilenMedyaGetir(type);
    var videoYolu = secim.videoPath || '';
    var gorselAdi = secim.media.length > 0 ? medyaYolAdi(secim.media[0]) : 'Medya (simule)';
    var siraKayit = (type === 'kampanya' || type === 'detayli') ? '001' : siraNo;

    gecmisSMKayitEkle({
        tarihSaat: new Date().toLocaleString('tr-TR'),
        tur: names[type],
        siraNumarasi: siraKayit,
        baslik: baslik,
        gorselAdi: gorselAdi,
        platform: 'Instagram, Facebook, LinkedIn, X, TikTok, YouTube',
        sablon: 'Standart',
        platformCikti: 'Platforma ozel duzenleme (simule)',
        durum: 'bekliyor',
        icerik: mesaj,
        baglanti: '',
        hataNedeni: ''
    });

    if (type === 'standart') {
        otomatikStandartEkle(baslik, siraNo, gorselAdi, videoYolu, mesaj, secim.media);
        alert('Standart Paylasim kaydedildi. Sira numarasi: ' + siraNo + '. Otomatik yayin sirasina eklendi ve mevcut sira numarasi ile dongude kalacak.');
    } else if (type === 'kampanya') {
        var basTarih = document.getElementById('kampanyaBaslangic') ? document.getElementById('kampanyaBaslangic').value : '';
        var bitTarih = document.getElementById('kampanyaBitis') ? document.getElementById('kampanyaBitis').value : '';
        if (!basTarih) { basTarih = new Date().toISOString().split('T')[0]; }
        if (!bitTarih) { var b = new Date(); b.setDate(b.getDate() + 30); bitTarih = b.toISOString().split('T')[0]; }
        otomatikKampanyaEkle(baslik, basTarih, bitTarih, gorselAdi, videoYolu, mesaj, secim.media);
        alert('Kampanya Paylasimi kaydedildi. Kampanya baslangic ve bitis tarihleri arasinda otomatik yayinlanacak. Standart Paylasim dongusune dahil edilmez.');
    } else {
        var tarih = document.getElementById('detayliTarih') ? document.getElementById('detayliTarih').value : '';
        var saat = document.getElementById('detayliSaat') ? document.getElementById('detayliSaat').value : '';
        otomatikDetayliEkle(baslik, mesaj, gorselAdi, videoYolu, secim.media, tarih, saat);
        alert('Detayli Paylasim planlandi. Zamanlayici belirtilen tarih/saatte yayinlar.');
    }
}

// ===== GERCEK YAYIN MOTORU (Facebook / Instagram / TikTok / X / LinkedIn) =====
// Manuel "Simdi Paylas" yayin motoruna baglanan gercek yayin sevkiyatcisi.
// Bagli Facebook/Instagram/TikTok/X/LinkedIn hesaplarina gercek Tauri
// komutlarini (facebook_publish, instagram_publish, tiktok_publish, x_publish,
// linkedin_publish) cagirir. Sonuclar gercek post id ile
// basarili, aksi halde kontrollu hata koduyla basarisiz olarak islenir; sahte
// basari uretilmez.
// Bagli yayin destekli hesap yoksa hicbir platform icin basari iddia edilmez.
function sosyalGercekYayinGonder(icerik) {
    var sonuc = {
        platformlar: [], // { platformId, basarili, postId, hataMesaji }
        toplamBasarili: 0,
        toplamBasarisiz: 0,
        bagliHesapYok: false
    };

    // Bagli hesaplari gercek deposundan al (token'lar frontend'e gelmez).
    var p = esTauriInvoke('social_account_connections');
    if (!p) {
        // Tauri ortami yok: gercek yayin yapilamaz; sahte basari uretilmez.
        sonuc.bagliHesapYok = true;
        return Promise.resolve(sonuc);
    }

    return p.then(function(list) {
        var bagli = (list || []).filter(function(c) {
            return c.connectionStatus === 'connected';
        });
        // Yalniz yayin destekli platformlar hedeflenir: Facebook, Instagram,
        // TikTok, X, LinkedIn.
        var yayinBagli = bagli.filter(function(c) {
            return c.platformId === 'facebook' || c.platformId === 'instagram' || c.platformId === 'tiktok' || c.platformId === 'x' || c.platformId === 'linkedin';
        });

        if (yayinBagli.length === 0) {
            sonuc.bagliHesapYok = true;
            return sonuc;
        }

        var istekler = yayinBagli.map(function(conn) {
            var platformId = conn.platformId;
            var command;
            var args;
            if (platformId === 'tiktok') {
                // TikTok video yayini: gerçek Content Posting API (video init +
                // presigned upload + durum yoklamasi). Gizlilik kontrolü bir
                // değerle gelir; video dosya yolu icerikten alinir.
                command = 'tiktok_publish';
                args = {
                    connectionId: conn.connectionId,
                    videoPath: icerik.videoPath || '',
                    title: icerik.baslik || icerik.mesaj || '',
                    privacyLevel: icerik.privacyLevel || 'SELF_ONLY'
                };
            } else if (platformId === 'x') {
                // X (Twitter) yayini: gercek API (media upload + tweet.create).
                command = 'x_publish';
                args = {
                    connectionId: conn.connectionId,
                    videoPath: icerik.videoPath || '',
                    title: icerik.baslik || icerik.mesaj || ''
                };
            } else if (platformId === 'facebook') {
                command = 'facebook_publish';
                args = {
                    connectionId: conn.connectionId,
                    message: icerik.mesaj || '',
                    title: icerik.baslik || '',
                    mediaKind: icerik.mediaKind || '',
                    mediaFiles: icerik.mediaFiles || []
                };
            } else if (platformId === 'linkedin') {
                // LinkedIn yayini: gercek Posts API (metin) / Images API
                // (gorsel) / Videos API (video). Eski UGC/Share API kullanilmaz.
                command = 'linkedin_publish';
                args = {
                    connectionId: conn.connectionId,
                    message: icerik.mesaj || '',
                    title: icerik.baslik || '',
                    mediaKind: icerik.mediaKind || '',
                    mediaFiles: icerik.mediaFiles || []
                };
            } else {
                command = 'instagram_publish';
                args = {
                    connectionId: conn.connectionId,
                    caption: icerik.mesaj || '',
                    mediaKind: icerik.mediaKind || '',
                    mediaUrls: icerik.mediaUrls || [],
                    postKind: 'feed'
                };
            }
            return esTauriInvoke(command, args).then(function(id) {
                var asilId = (typeof id === 'string' && id) ? id : '';
                return { platformId: platformId, basarili: true, postId: asilId };
            }).catch(function(err) {
                var raw = (err && (err.message || err.code || err)) || '';
                return { platformId: platformId, basarili: false, hataMesaji: metaHataMesaji(String(raw)) };
            });
        });

        return Promise.all(istekler).then(function(parca) {
            parca.forEach(function(r) {
                if (r.basarili) { sonuc.toplamBasarili++; } else { sonuc.toplamBasarisiz++; }
                sonuc.platformlar.push(r);
            });
            return sonuc;
        });
    }).catch(function() {
        sonuc.bagliHesapYok = true;
        return sonuc;
    });
}

// Seçili video dosyasının gerçek mutlak yolunu native dosya seçici (dialog)
// üzerinden Rust'dan alır. Tarayıcı güvenliği gereği ön yüz gerçek yola
// erişemediği için bu komut kullanılır. Video seçilmediyse veya kullanıcı
// iptal ederse boş dize döner (sahte ön yol üretilmez).
function videoDosyaYoluAl() {
    var p = esTauriInvoke('pick_video_file');
    if (!p) return Promise.resolve(''); // Tauri ortamı yok: gerçek yol alınamaz
    return p.catch(function() { return ''; });
}

// Secilen dosyalarin video olup olmadigini uzantidan anlar.
function seciliVideoMi(inputEl) {
    if (!inputEl || !inputEl.files || inputEl.files.length === 0) return false;
    var name = inputEl.files[0].name.toLowerCase();
    return /\.(mp4|mov|avi|mkv|webm|m4v|flv|3gp|mpeg|mpg|ogv|ts|wmv)$/.test(name);
}

function simulateNow(type) {
    var names = {
        'standart': 'Standart Paylasim',
        'kampanya': 'Kampanya Paylasimi',
        'detayli': 'Detayli Paylasim',
        'duyuru': 'Duyuru ve Ilanlar'
    };
    var siraNo = (type === 'duyuru') ? '' : String(Math.floor(Math.random() * 899) + 100);

    // Secilen medya dosyalarini topla (yalnizca ad; icerik bilgisi frontend'de
    // tutulmaz, Rust tarafi gercek dosya yolunu diskten mulkiyetinde tutar).
    var inputEl = document.getElementById(type + 'FileInput');
    var seciliDosyalar = [];
    var medyaVar = false;
    if (inputEl && inputEl.files && inputEl.files.length > 0) {
        medyaVar = true;
        seciliDosyalar = Array.from(inputEl.files).map(function(f) { return f.name; });
    }

    // TikTok video için gerçek disk yolunu native dosya seçici ile çöz.
    // Video seçildiğinde (uzantıya göre) iptal edilirse boş dizeyle devam edilir.
    var videoSecildi = seciliVideoMi(inputEl);
    return (videoSecildi ? videoDosyaYoluAl() : Promise.resolve('')).then(function(gercekYol) {
        var videoPath = gercekYol || '';

        var postKapsami = {
            mesaj: names[type] + ' icerigi (manuel yayin)',
            baslik: names[type],
            medyaVar: medyaVar,
            mediaKind: medyaVar ? 'image' : '',
            mediaFiles: seciliDosyalar,
            mediaUrls: [],
            videoPath: videoPath,
            privacyLevel: 'SELF_ONLY'
        };

        // Gercek Facebook/Instagram/TikTok yayinlarini baslat; bagli hesap yoksa
        // ya da yayin basarisizsa sahte basari uretilmez (Yayin Gecmisi'ne yansitilir).
        return sosyalGercekYayinGonder(postKapsami).then(function(bilgi) {
        if (bilgi.platformlar.length === 0) {
            gecmisSMKayitEkle({
                tarihSaat: new Date().toLocaleString('tr-TR'),
                tur: names[type] + ' (Manuel)',
                siraNumarasi: siraNo,
                baslik: names[type] + ' manuel yayini',
                gorselAdi: medyaVar ? ('Gorsel dosyasi (' + seciliDosyalar.length + ')') : 'Medya yok',
                platform: 'Facebook, Instagram, TikTok (bagli hesap yok)',
                sablon: 'Standart',
                platformCikti: 'Gercek yayin yapilamadi (bagli hesap yok)',
                durum: 'basarisiz',
                icerik: names[type] + ' icerigi yayinlanamadi: bagli Facebook/Instagram/TikTok hesabi bulunamadi.',
                baglanti: '',
                hataNedeni: 'Bagli Facebook/Instagram/TikTok hesabi bulunmadi. Gercek yayin yapilamadi.'
            });
            alert(names[type] + ' yayinlanamadi. Bagli Facebook, Instagram veya TikTok hesabi bulunmadi; sahte basari uretilmedi.');
            return;
        }

        bilgi.platformlar.forEach(function(r) {
            gecmisSMKayitEkle({
                tarihSaat: new Date().toLocaleString('tr-TR'),
                tur: names[type] + ' (Manuel)',
                siraNumarasi: siraNo,
                baslik: r.platformId.toUpperCase() + ' manuel yayini',
                gorselAdi: medyaVar ? ('Gorsel (' + seciliDosyalar.length + ')') : 'Medya yok',
                platform: r.platformId,
                sablon: 'Standart',
                platformCikti: r.basarili ? ('Yayin ID: ' + (r.postId || 'basarili')) : 'Hedef platform yayini kabul etmedi',
                durum: r.basarili ? 'basarili' : 'basarisiz',
                icerik: names[type] + ' manuel yayini ' + (r.basarili ? 'basarili.' : 'basarisiz.'),
                baglanti: r.basarili ? ('Gercek yayin ID: ' + (r.postId || '-')) : '',
                hataNedeni: r.basarili ? '' : (r.hataMesaji || 'Yayin basarisiz.')
            });
            if (r.basarili) {
                bildirimEkle('sosyal-medya-baglanti', 'basarili',
                    r.platformId + ' yayini basarili',
                    r.platformId + ' manuel yayin gerceklestirildi (ID: ' + (r.postId || '-') + ').');
            } else {
                bildirimEkle('yayin-hatasi', 'hata',
                    r.platformId + ' yayini basarisiz',
                    r.hataMesaji || 'Yayin gerceklestirilemedi.');
            }
        });

        var ozet;
        if (bilgi.toplamBasarili > 0 && bilgi.toplamBasarisiz === 0) {
            ozet = names[type] + ' bagli Facebook, Instagram ve TikTok hesaplarinda basariyla yayinlandi.';
        } else if (bilgi.toplamBasarili > 0) {
            ozet = names[type] + ' kismen yayinlandi (' + bilgi.toplamBasarili + ' basarili, ' + bilgi.toplamBasarisiz + ' basarisiz).';
        } else {
            ozet = names[type] + ' bagli Facebook, Instagram ve TikTok hesaplarina yayinlanamadi (' + bilgi.toplamBasarisiz + ' basarisiz).';
        }
        alert(ozet);
        });
    });
}

// ===== FAZ 3 - SOSYAL MEDYA / WEB SITESI SEKME DEGISIMI =====
function switchSMWebTab(tab) {
    document.querySelectorAll('.paylasim-tabs:not(.sm-tabs) .tab').forEach(function(el) {
        el.classList.remove('active');
        if (el.getAttribute('data-paylasim-tab') === tab) {
            el.classList.add('active');
        }
    });

    var sosyal = document.getElementById('sm-web-sosyal');
    var web = document.getElementById('sm-web-web');
    if (tab === 'sosyal-medya') {
        sosyal.style.display = 'block';
        web.style.display = 'none';
    } else {
        sosyal.style.display = 'none';
        web.style.display = 'block';
    }
}

// ===== FAZ 3 - WEB DOSYA YUKLEME =====
document.addEventListener('DOMContentLoaded', function() {
    var webArea = document.getElementById('webUploadArea');
    var webInput = document.getElementById('webFileInput');
    var webContainer = document.getElementById('webUploadedFiles');
    if (webArea && webInput && webContainer) {
        webArea.addEventListener('click', function() { webInput.click(); });
        webInput.addEventListener('change', function() {
            webContainer.innerHTML = '';
            webSecilenMedyaPath = '';
            webSecilenMedyaAd = '';
            var files = Array.from(this.files);
            files.forEach(function(file, index) {
                var item = document.createElement('div');
                item.className = 'uploaded-file-item';
                item.innerHTML = '<span class="file-name">' + (index + 1) + '. ' + file.name + '</span><span class="file-remove" onclick="this.parentElement.remove()">Kaldir</span>';
                webContainer.appendChild(item);
            });
        });
        webArea.addEventListener('dragover', function(e) {
            e.preventDefault();
            webArea.style.borderColor = '#4f8cff';
            webArea.style.background = '#f0f4ff';
        });
        webArea.addEventListener('dragleave', function() {
            webArea.style.borderColor = '#d1d5db';
            webArea.style.background = '#f9fafb';
        });
        webArea.addEventListener('drop', function(e) {
            e.preventDefault();
            webArea.style.borderColor = '#d1d5db';
            webArea.style.background = '#f9fafb';
            webContainer.innerHTML = '';
            webSecilenMedyaPath = '';
            webSecilenMedyaAd = '';
            var files = Array.from(e.dataTransfer.files);
            files.forEach(function(file, index) {
                var item = document.createElement('div');
                item.className = 'uploaded-file-item';
                item.innerHTML = '<span class="file-name">' + (index + 1) + '. ' + file.name + '</span><span class="file-remove" onclick="this.parentElement.remove()">Kaldir</span>';
                webContainer.appendChild(item);
            });
            webInput.files = e.dataTransfer.files;
        });
    }
});

// ===== FAZ 3 - WEB TASLAK YONETIMI =====
var webTaslaklar = [];

function webTaslakKaydet() {
    var icerik = document.getElementById('webIcerik').value.trim();
    var baslik = document.getElementById('webBaslik').value.trim();
    var bolum = document.getElementById('webBolum').value;

    if (icerik === '' && baslik === '') {
        alert('Yayinlanacak icerik veya baslik girin.');
        return;
    }

    var bolumAdiEl = document.getElementById('webBolumAdi');
    var bolumAdi = bolumAdiEl ? bolumAdiEl.value.trim() : '';
    if (!bolumAdi) bolumAdi = bolum !== '' ? bolum : 'Belirtilmemis';
    var taslak = {
        id: Date.now(),
        icerik: icerik,
        baslik: baslik,
        bolum: bolum,
        bolumAdi: bolumAdi,
        tarih: new Date().toLocaleString('tr-TR')
    };
    webTaslaklar.push(taslak);
    webTaslakListele();
    alert('Taslak kaydedildi.');

    document.getElementById('webIcerik').value = '';
    document.getElementById('webBaslik').value = '';
    document.getElementById('webBolum').value = '';
    if (document.getElementById('webBolumAdi')) document.getElementById('webBolumAdi').value = '';
    var wc = document.getElementById('webUploadedFiles');
    if (wc) wc.innerHTML = '';
}

function webTaslakListele() {
    var alan = document.getElementById('webTaslaklarArea');
    var liste = document.getElementById('webTaslakListesi');
    if (!alan || !liste) return;

    if (webTaslaklar.length === 0) {
        alan.style.display = 'none';
        return;
    }
    alan.style.display = 'block';
    liste.innerHTML = '';

    webTaslaklar.forEach(function(t, index) {
        var item = document.createElement('div');
        item.className = 'taslak-item';
        item.innerHTML = '<div class="taslak-info"><div class="taslak-baslik">' +
            (t.baslik || '(Basliksiz)') +
            '</div><div class="taslak-meta">Bolum: ' + t.bolumAdi + ' | ' + t.tarih + '</div></div>' +
            '<div class="taslak-actions">' +
            '<button class="btn-duzenle" onclick="webTaslakDuzenle(' + index + ')">Duzenle</button>' +
            '<button class="btn-yayinla" onclick="webTaslakYayinla(' + index + ')">Yayinla</button>' +
            '<button class="btn-sil" onclick="webTaslakSil(' + index + ')">Sil</button>' +
            '</div>';
        liste.appendChild(item);
    });
}

function webTaslakDuzenle(index) {
    var t = webTaslaklar[index];
    document.getElementById('webIcerik').value = t.icerik;
    document.getElementById('webBaslik').value = t.baslik;
    document.getElementById('webBolum').value = t.bolum;
    if (document.getElementById('webBolumAdi')) document.getElementById('webBolumAdi').value = t.bolumAdi || t.bolum;
    webTaslaklar.splice(index, 1);
    webTaslakListele();
}

function webTaslakSil(index) {
    webTaslaklar.splice(index, 1);
    webTaslakListele();
}

function webTaslakYayinla(index) {
    var t = webTaslaklar[index];
    if (!t.bolum || t.bolum === '') {
        alert('Bu taslagin hedef web bolumu secilmemistir. Lutfen once duzenleyerek web bolumu secin.');
        return;
    }
    document.getElementById('webBolum').value = t.bolum;
    if (document.getElementById('webBolumAdi')) document.getElementById('webBolumAdi').value = t.bolumAdi || t.bolum;
    webYayinla(t.icerik, t.baslik, t.bolum);
}

// ===== FAZ 3 - WEB SIMDI YAYINLA =====
var webSecilenMedyaPath = '';
var webSecilenMedyaAd = '';

function webSimdiYayinla() {
    var icerik = document.getElementById('webIcerik').value.trim();
    var baslik = document.getElementById('webBaslik').value.trim();
    var bolum = document.getElementById('webBolum').value.trim();

    if (icerik === '' && baslik === '') {
        alert('Yayinlanacak icerik veya baslik girin.');
        return;
    }
    if (!bolum || bolum === '') {
        alert('Web bolumu zorunludur. Bolum kimligini yazin veya bolumleri API\'den getirerek secin.');
        return;
    }

    webYayinla(icerik, baslik, bolum);
}

function webYayinla(icerik, baslik, bolum) {
    var bolumAdiEl = document.getElementById('webBolumAdi');
    var bolumAdi = bolumAdiEl ? bolumAdiEl.value.trim() : '';
    if (!bolumAdi) bolumAdi = bolum;

    var p = esTauriInvoke('website_publish', {
        title: baslik || '',
        content: icerik,
        section_id: bolum || '',
        section_name: bolumAdi || '',
        media_path: webSecilenMedyaPath || ''
    });
    if (!p) {
        alert('Web yayinlama yalniz uygulama ici calistiginda yapilabilir. Ayarlar > Web Sitesi b\u00f6l\u00fcm\u00fcnden API bilgilerini girin.');
        return;
    }
    p.then(function(out) {
        if (out && out.ok) {
            var mesaj = 'Web yayini basarili.\n' + out.turkce;
            if (out.content_id) mesaj += '\nIcerik ID: ' + out.content_id;
            if (out.content_url) mesaj += '\nAdres: ' + out.content_url;
            alert(mesaj);
            bildirimEkle('web-baglanti', 'basarili', 'Web yayini yayinlandi', out.turkce);
            gecmisWebKayitEkle({
                durum: 'basarili',
                tarihSaat: new Date().toLocaleString('tr-TR'),
                tur: 'Web Yayini',
                webBolum: bolumAdi || bolum,
                baslik: baslik || '(baslik yok)',
                gorselAdi: webSecilenMedyaAd || '-',
                icerik: icerik,
                icerikId: out.content_id || '',
                icerikAdres: out.content_url || ''
            });
            webFormuTemizle();
        } else {
            var msg = out ? out.turkce : 'Yayin sonucu alinamadi.';
            alert('Web yayini basarisiz.\n' + msg);
            bildirimEkle('web-baglanti', 'uyari', 'Web yayini basarisiz', msg);
            gecmisWebKayitEkle({
                durum: 'basarisiz',
                tarihSaat: new Date().toLocaleString('tr-TR'),
                tur: 'Web Yayini',
                webBolum: bolumAdi || bolum,
                baslik: baslik || '(baslik yok)',
                gorselAdi: webSecilenMedyaAd || '-',
                icerik: icerik,
                hataNedeni: msg
            });
        }
    }).catch(function(e) {
        var msg = (e && e.message) ? e.message : (typeof e === 'string' ? e : 'Bilinmeyen hata');
        alert('Web yayini basarisiz.\n' + msg);
        bildirimEkle('web-baglanti', 'uyari', 'Web yayini basarisiz', msg);
    });
}

function webFormuTemizle() {
    if (document.getElementById('webIcerik')) document.getElementById('webIcerik').value = '';
    if (document.getElementById('webBaslik')) document.getElementById('webBaslik').value = '';
    if (document.getElementById('webBolum')) document.getElementById('webBolum').value = '';
    if (document.getElementById('webBolumAdi')) document.getElementById('webBolumAdi').value = '';
    var wc = document.getElementById('webUploadedFiles');
    if (wc) wc.innerHTML = '';
    webSecilenMedyaPath = '';
    webSecilenMedyaAd = '';
}

function webDisktenSec() {
    var p = esTauriInvoke('pick_media_files');
    if (!p) {
        alert('Dosya secimi yalniz uygulama ici calistiginda yapilabilir. Onizleme modunda surukle-birak alanini kullanin.');
        return;
    }
    p.then(function(paths) {
        if (!paths || paths.length === 0) return;
        webSecilenMedyaPath = paths[0];
        var ad = webSecilenMedyaPath.split(/[\\/]/).pop();
        webSecilenMedyaAd = ad;
        var wc = document.getElementById('webUploadedFiles');
        if (wc) wc.innerHTML = '<div class="uploaded-file-item"><span class="file-name">' + ad + '</span></div>';
    }).catch(function(e) {
        var msg = (e && e.message) ? e.message : 'Bilinmeyen hata';
        alert('Dosya secilemedi.\n' + msg);
    });
}

function webBolumleriGetir() {
    var p = esTauriInvoke('website_sections');
    if (!p) {
        alert('Bolum listesi yalniz uygulama ici calistiginda API\'den alinabilir. Bolum kimligini elle girebilirsiniz.');
        return;
    }
    p.then(function(out) {
        if (!out || !out.ok) {
            var msg = out ? out.turkce : 'Bolum listesi alinamadi.';
            alert(msg);
            bildirimEkle('web-baglanti', 'uyari', 'Bolum listesi alinamadi', msg);
            return;
        }
        var dl = document.getElementById('webBolumListesi');
        if (dl) {
            dl.innerHTML = '';
            out.sections.forEach(function(s) {
                var opt = document.createElement('option');
                opt.value = s.id;
                opt.label = s.name;
                dl.appendChild(opt);
            });
        }
        var ip = document.getElementById('webBolum');
        if (ip && ip.value && out.sections.length > 0) {
            var eslesme = out.sections.filter(function(s) { return s.id === ip.value; });
            var adEl = document.getElementById('webBolumAdi');
            if (eslesme.length > 0 && adEl) adEl.value = eslesme[0].name;
        }
        alert(out.turkce);
        bildirimEkle('web-baglanti', 'basarili', 'Bolum listesi alindi', out.turkce);
    }).catch(function(e) {
        var msg = (e && e.message) ? e.message : (typeof e === 'string' ? e : 'Bilinmeyen hata');
        alert('Bolum listesi alinamadi.\n' + msg);
    });
}

document.addEventListener('input', function(e) {
    if (e.target && e.target.id === 'webBolum') {
        var adEl = document.getElementById('webBolumAdi');
        if (!adEl) return;
        var dl = document.getElementById('webBolumListesi');
        if (!dl) return;
        var secili = Array.prototype.filter.call(dl.options, function(o) { return o.value === e.target.value; });
        if (secili.length > 0) {
            adEl.value = secili[0].label || secili[0].text;
        }
    }
});

// ===== FAZ 4 - MEDYA KUTUPHANESI =====
var medyaSeciliKlasor = '01';
var medyaSeciliAltKlasor = '';
var medyaYuklenenDosyaKayitlari = {};

var medyaKlasorYapisi = {
    '01': { ad: '01_KAYNAK_GORSELLER', alt: ['Urunler','Hizmetler','Magaza_Ofis','Personel','Etkinlikler','Kampanyalar','Referanslar','Marka_Gorselleri','Diger'] },
    '02': { ad: '02_SOSYAL_MEDYA_GORSELLERI', alt: ['01_Gunluk_Paylasimlar','02_Haftalik_Paylasimlar','03_Aylik_Paylasimlar','04_Ozel_Gunler','05_Kampanyalar','06_Yeni_Urunler','07_Hizmet_Tanitimlari','08_Duyurular','09_Kurumsal_Paylasimlar','10_Bilgilendirici_Icerikler','11_Musteri_Yorumlari','12_Etkinlikler','13_Sezonluk_Paylasimlar','14_Diger'] },
    '03': { ad: '03_WEB_GORSELLERI', alt: ['01_Ana_Sayfa','02_Slider_Banner','03_Hero_Gorselleri','04_Urun_Gorselleri','05_Kategori_Gorselleri','06_Hizmet_Gorselleri','07_Kampanya_Bannerlari','08_Blog_Gorselleri','09_Hakkimizda','10_Referanslar','11_Ekip_Gorselleri','12_Popup_Gorselleri','13_Mobil_Web_Gorselleri','14_Diger'] },
    '04': { ad: '04_LOGO_VE_KURUMSAL_KIMLIK', alt: [] },
    '05': { ad: '05_SABLONLAR', alt: ['SOSYAL_MEDYA_SABLONLARI','WEB_SABLONLARI'] }
};

var medyaDosyalari = {};

var medyaSablonAltKlasorler = {
    'SOSYAL_MEDYA_SABLONLARI': ['Dikey_Gonderi','Kare_Gonderi','Story','Reels_Kapagi','Yatay_Gonderi','Platforma_Ozel'],
    'WEB_SABLONLARI': ['Masaustu_Hero','Mobil_Hero','Yatay_Banner','Kampanya_Banneri','Blog_Kapagi','Urun_Karti','Kategori_Gorseli']
};

var medyaOzelGunAltKlasorler = ['Resmi_Bayramlar','Dini_Bayramlar','Milli_Gunler','Mesleki_Gunler','Sektorel_Gunler','Yilbasi','Anneler_Gunu','Babalar_Gunu','Sevgililer_Gunu','Firmaya_Ozel_Gunler'];

// Sablon tipi: 'sosyal-medya' veya 'web'
var medyaSablonTipleri = {
    'SOSYAL_MEDYA_SABLONLARI': 'sosyal-medya',
    'WEB_SABLONLARI': 'web'
};

// Dosya icin benzersiz anahtar olustur (ad + boyut + lastModified)
function medyaDosyaAnahtari(file) {
    return file.name.toLowerCase().trim() + '|' + file.size + '|' + (file.lastModified || '0');
}

function medyaDosyaAdiAnahtari(ad, boyut, sonDegisim) {
    return ad.toLowerCase().trim() + '|' + (boyut || '0') + '|' + (sonDegisim || '0');
}

function medyaVideoKontrolEt(file) {
    var videoUzantilari = ['.mp4', '.avi', '.mov', '.wmv', '.flv', '.mkv', '.webm', '.m4v', '.mpg', '.mpeg', '.3gp', '.ogv', '.ts'];
    var fileName = file.name.toLowerCase();
    var isVideo = videoUzantilari.some(function(ext) { return fileName.endsWith(ext); });
    var isVideoMime = file.type && file.type.startsWith('video/');
    if (isVideo || isVideoMime) {
        alert('Video dosyalari Medya Kutuphanesinde saklanamaz. Lutfen yalnizca gorsel dosyasi yukleyin.');
        return true;
    }
    return false;
}

function medyaAyniDosyaKontrolEt(file) {
    var anahtar = medyaDosyaAnahtari(file);
    if (medyaYuklenenDosyaKayitlari[anahtar]) {
        alert('"' + file.name + '" dosyasi (' + file.size + ' byte) daha once baska bir klasore yuklenmistir. Ayni dosya farkli klasorlere kopyalanamaz.');
        return true;
    }
    return false;
}

function medyaSablonKullanimaUygunMu(altKlasorAdi, hedefTip) {
    var altKlasorTemiz = altKlasorAdi.trim();
    if (altKlasorTemiz === '') return true;

    var ebeveynTip = null;
    var ebeveynBulundu = false;

    // Hangi ana sablon grubuna ait oldugunu bul
    var smSablonlari = medyaSablonAltKlasorler['SOSYAL_MEDYA_SABLONLARI'];
    var webSablonlari = medyaSablonAltKlasorler['WEB_SABLONLARI'];

    if (smSablonlari && smSablonlari.indexOf(altKlasorTemiz) !== -1) {
        ebeveynTip = 'sosyal-medya';
        ebeveynBulundu = true;
    } else if (webSablonlari && webSablonlari.indexOf(altKlasorTemiz) !== -1) {
        ebeveynTip = 'web';
        ebeveynBulundu = true;
    }

    if (!ebeveynBulundu) return true; // Sablon degil, engel yok

    if (ebeveynTip !== hedefTip) {
        alert('Bu sablon "' + altKlasorTemiz + '" ' + (ebeveynTip === 'sosyal-medya' ? 'sosyal medya' : 'web') + ' sablonudur ve ' + (hedefTip === 'web' ? 'web' : 'sosyal medya') + ' sablonu olarak kullanilamaz.');
        return false;
    }
    return true;
}

function medyaKlasorSec(klasorId) {
    medyaSeciliKlasor = klasorId;
    medyaSeciliAltKlasor = '';

    document.querySelectorAll('.medya-klasor').forEach(function(el) {
        el.classList.remove('active');
        if (el.getAttribute('data-klasor') === klasorId) {
            el.classList.add('active');
        }
    });

    var sablonBilgi = document.getElementById('medyaSablonBilgi');
    if (sablonBilgi) {
        sablonBilgi.style.display = (klasorId === '05') ? 'block' : 'none';
    }

    var yuklemeAlani = document.getElementById('medyaYuklemeAlani');
    if (yuklemeAlani) {
        yuklemeAlani.style.display = 'block';
    }

    medyaAltKlasorleriGoster(klasorId);
    medyaDosyalariListele();
}

function medyaAltKlasorleriGoster(klasorId) {
    var alan = document.getElementById('medyaAltKlasorler');
    var bilgi = medyaKlasorYapisi[klasorId];
    if (!bilgi) { alan.innerHTML = ''; return; }

    var altListe = [];
    if (klasorId === '02') {
        bilgi.alt.forEach(function(a) {
            altListe.push(a);
            if (a === '04_Ozel_Gunler') {
                medyaOzelGunAltKlasorler.forEach(function(og) { altListe.push('  -> ' + og); });
            }
        });
    } else if (klasorId === '05') {
        bilgi.alt.forEach(function(a) {
            altListe.push(a);
            var icKlasorler = medyaSablonAltKlasorler[a];
            if (icKlasorler) { icKlasorler.forEach(function(ik) { altListe.push('  -> ' + ik); }); }
        });
    } else {
        altListe = bilgi.alt;
    }

    if (altListe.length === 0) {
        alan.innerHTML = '<div style="font-size:0.85rem;color:#9ca3af;">Bu klasorde alt klasor bulunmamaktadir.</div>';
        return;
    }

    var html = '';
    altListe.forEach(function(a) {
        var isChild = a.startsWith('  -> ');
        var displayName = isChild ? a.substring(4) : a;
        var cssClass = isChild ? 'medya-alt-klasor child' : 'medya-alt-klasor';
        var activeClass = (medyaSeciliAltKlasor === a.trim()) ? ' active' : '';
        html += '<div class="' + cssClass + activeClass + '" onclick="medyaAltKlasorSec(\'' + a.trim().replace(/'/g, "\\'") + '\')">' + displayName + '</div>';
    });
    alan.innerHTML = html;
}

function medyaAltKlasorSec(altKlasor) {
    medyaSeciliAltKlasor = altKlasor;
    document.querySelectorAll('.medya-alt-klasor').forEach(function(el) {
        el.classList.remove('active');
        if (el.textContent.trim() === altKlasor) { el.classList.add('active'); }
    });
    medyaDosyalariListele();
}

function medyaDosyalariListele() {
    var alan = document.getElementById('medyaDosyaListesi');
    var anahtar = medyaSeciliKlasor + '/' + medyaSeciliAltKlasor;
    var dosyalar = medyaDosyalari[anahtar] || [];

    if (dosyalar.length === 0) {
        alan.innerHTML = '<div class="medya-bos">Bu klasorde henuz dosya bulunmamaktadir.</div>';
        return;
    }

    var html = '<div style="display:flex;flex-wrap:wrap;gap:12px;">';
    dosyalar.forEach(function(d, i) {
        html += '<div style="width:120px;padding:12px;background:#f9fafb;border:1px solid #e5e7eb;border-radius:6px;text-align:center;"><div style="font-size:2rem;margin-bottom:6px;">🖼️</div><div style="font-size:0.72rem;color:#374151;word-break:break-all;">' + d.ad + '</div><div style="font-size:0.65rem;color:#9ca3af;margin-top:4px;">' + d.tarih + '</div><div style="margin-top:6px;"><span class="file-remove" onclick="medyaDosyaSil(' + i + ')">Sil</span></div></div>';
    });
    html += '</div>';
    alan.innerHTML = html;
}

function medyaDosyaYukle() {
    var container = document.getElementById('medyaUploadedFiles');
    var dosyaSayisi = container ? container.children.length : 0;

    if (dosyaSayisi === 0) {
        alert('Lutfen once bir dosya secin.');
        return;
    }

    var yuklenecekler = [];
    var fileItems = container.querySelectorAll('.uploaded-file-item');
    fileItems.forEach(function(item) {
        var fileNameEl = item.querySelector('.file-name');
        if (fileNameEl) {
            var ad = fileNameEl.textContent.replace(/^\d+\.\s*/, '');
            var boyut = item.getAttribute('data-size') || '0';
            var sonDegisim = item.getAttribute('data-lastmodified') || '0';
            yuklenecekler.push({ ad: ad, boyut: boyut, sonDegisim: sonDegisim });
        }
    });

    var anahtar = medyaSeciliKlasor + '/' + medyaSeciliAltKlasor;
    if (!medyaDosyalari[anahtar]) { medyaDosyalari[anahtar] = []; }

    yuklenecekler.forEach(function(d) {
        var kayitAnahtari = medyaDosyaAdiAnahtari(d.ad, d.boyut, d.sonDegisim);
        medyaDosyalari[anahtar].push({ ad: d.ad, boyut: d.boyut, sonDegisim: d.sonDegisim, anahtar: kayitAnahtari, tarih: new Date().toLocaleString('tr-TR') });
        medyaYuklenenDosyaKayitlari[kayitAnahtari] = true;
    });

    container.innerHTML = '';
    medyaDosyalariListele();
    alert('Dosya(lar) yuklendi. Ayni dosya farkli klasorlere kopyalanmaz, tek ana kayit mantigi ile yonetilir.');
}

function medyaDosyaSil(index) {
    var anahtar = medyaSeciliKlasor + '/' + medyaSeciliAltKlasor;
    if (medyaDosyalari[anahtar]) {
        var silinecek = medyaDosyalari[anahtar][index];
        if (silinecek && silinecek.anahtar && medyaYuklenenDosyaKayitlari) {
            delete medyaYuklenenDosyaKayitlari[silinecek.anahtar];
        }
        medyaDosyalari[anahtar].splice(index, 1);
        medyaDosyalariListele();
    }
}

// MEDYA file upload DOM
document.addEventListener('DOMContentLoaded', function() {
    var medyaArea = document.getElementById('medyaFileUploadArea');
    var medyaInput = document.getElementById('medyaFileInput');
    var medyaContainer = document.getElementById('medyaUploadedFiles');

    if (medyaArea && medyaInput && medyaContainer) {
        medyaArea.addEventListener('click', function() { medyaInput.click(); });

        function medyaDosyaEkle(files) {
            medyaContainer.innerHTML = '';
            var index = 0;
            files.forEach(function(file) {
                // Video engelle
                if (medyaVideoKontrolEt(file)) return;
                // Ayni dosya engelle (ad + boyut + lastModified)
                if (medyaAyniDosyaKontrolEt(file)) return;
                index++;
                var item = document.createElement('div');
                item.className = 'uploaded-file-item';
                item.setAttribute('data-size', file.size);
                item.setAttribute('data-lastmodified', file.lastModified || '0');
                item.innerHTML = '<span class="file-name">' + index + '. ' + file.name + '</span><span class="file-remove" onclick="this.parentElement.remove()">Kaldir</span>';
                medyaContainer.appendChild(item);
            });
        }

        medyaInput.addEventListener('change', function() {
            medyaDosyaEkle(Array.from(this.files));
        });

        medyaArea.addEventListener('dragover', function(e) {
            e.preventDefault();
            medyaArea.style.borderColor = '#4f8cff';
            medyaArea.style.background = '#f0f4ff';
        });

        medyaArea.addEventListener('dragleave', function() {
            medyaArea.style.borderColor = '#d1d5db';
            medyaArea.style.background = '#f9fafb';
        });

        medyaArea.addEventListener('drop', function(e) {
            e.preventDefault();
            medyaArea.style.borderColor = '#d1d5db';
            medyaArea.style.background = '#f9fafb';
            medyaDosyaEkle(Array.from(e.dataTransfer.files));
            medyaInput.files = e.dataTransfer.files;
        });
    }
});

// ===== FAZ 5 - YAYIN GECMISI =====
var yayinGecmisi = {
    sosyalMedya: [],
    webSitesi: []
};

function gecmisSMKayitEkle(kayit) {
    yayinGecmisi.sosyalMedya.unshift(kayit);
    gecmisSMListele();
    gecmisiKaydet();
}

function gecmisWebKayitEkle(kayit) {
    yayinGecmisi.webSitesi.unshift(kayit);
    gecmisWebListele();
    gecmisiKaydet();
}

function switchGecmisTab(tab) {
    document.querySelectorAll('.gecmis-tabs .tab').forEach(function(el) {
        el.classList.remove('active');
        if (el.getAttribute('data-gecmis-tab') === tab) {
            el.classList.add('active');
        }
    });
    var sm = document.getElementById('gecmis-sosyal-medya');
    var web = document.getElementById('gecmis-web-sitesi');
    if (tab === 'sosyal-medya') {
        sm.style.display = 'block';
        web.style.display = 'none';
    } else {
        sm.style.display = 'none';
        web.style.display = 'block';
    }
}

function gecmisSMFiltrele() {
    gecmisSMListele();
}

function gecmisWebFiltrele() {
    gecmisWebListele();
}

function gecmisSMListele() {
    var alan = document.getElementById('gecmisSMListe');
    if (!alan) return;
    var arama = document.getElementById('gecmisSMArama');
    var filtre = arama ? arama.value.toLowerCase().trim() : '';
    var kayitlar = yayinGecmisi.sosyalMedya;

    if (filtre !== '') {
        kayitlar = kayitlar.filter(function(k) {
            var baslik = (k.baslik || '').toLowerCase();
            var sira = (k.siraNumarasi || '').toString();
            return baslik.indexOf(filtre) !== -1 || sira.indexOf(filtre) !== -1;
        });
    }

    if (kayitlar.length === 0) {
        alan.innerHTML = '<div class="medya-bos">Henuz sosyal medya yayin kaydi bulunmamaktadir.</div>';
        return;
    }

    var html = '';
    kayitlar.forEach(function(k, i) {
        var durumClass = k.durum === 'basarili' ? 'durum-basarili' : (k.durum === 'basarisiz' ? 'durum-basarisiz' : 'durum-bekliyor');
        var durumText = k.durum === 'basarili' ? 'Basarili' : (k.durum === 'basarisiz' ? 'Basarisiz' : 'Bekliyor');
        var siraText = k.siraNumarasi ? k.siraNumarasi : '-';
        var baslikText = k.baslik || '(baslik yok)';

        html += '<div class="gecmis-kayit" onclick="gecmisSMDetayGoster(' + i + ')">';
        html += '    <div class="gecmis-durum"><span class="durum-badge ' + durumClass + '">' + durumText + '</span></div>';
        html += '    <div class="gecmis-bilgi">';
        html += '        <div class="gecmis-baslik">' + baslikText + '</div>';
        html += '        <div class="gecmis-meta">' + k.tarihSaat + ' | ' + k.tur + ' | Sira: ' + siraText + '</div>';
        html += '    </div>';
        html += '    <div class="gecmis-ok">&#9654;</div>';
        html += '</div>';
    });
    alan.innerHTML = html;
}

function gecmisWebListele() {
    var alan = document.getElementById('gecmisWebListe');
    if (!alan) return;
    var arama = document.getElementById('gecmisWebArama');
    var filtre = arama ? arama.value.toLowerCase().trim() : '';
    var kayitlar = yayinGecmisi.webSitesi;

    if (filtre !== '') {
        kayitlar = kayitlar.filter(function(k) {
            var baslik = (k.baslik || '').toLowerCase();
            return baslik.indexOf(filtre) !== -1;
        });
    }

    if (kayitlar.length === 0) {
        alan.innerHTML = '<div class="medya-bos">Henuz web sitesi yayin kaydi bulunmamaktadir.</div>';
        return;
    }

    var html = '';
    kayitlar.forEach(function(k, i) {
        var durumClass = k.durum === 'basarili' ? 'durum-basarili' : (k.durum === 'basarisiz' ? 'durum-basarisiz' : 'durum-bekliyor');
        var durumText = k.durum === 'basarili' ? 'Basarili' : (k.durum === 'basarisiz' ? 'Basarisiz' : 'Bekliyor');
        var baslikText = k.baslik || '(baslik yok)';
        var bolumText = k.webBolum || 'Belirtilmemis';

        html += '<div class="gecmis-kayit" onclick="gecmisWebDetayGoster(' + i + ')">';
        html += '    <div class="gecmis-durum"><span class="durum-badge ' + durumClass + '">' + durumText + '</span></div>';
        html += '    <div class="gecmis-bilgi">';
        html += '        <div class="gecmis-baslik">' + baslikText + '</div>';
        html += '        <div class="gecmis-meta">' + k.tarihSaat + ' | ' + bolumText + '</div>';
        html += '    </div>';
        html += '    <div class="gecmis-ok">&#9654;</div>';
        html += '</div>';
    });
    alan.innerHTML = html;
}

function gecmisSMDetayGoster(index) {
    var k = yayinGecmisi.sosyalMedya[index];
    if (!k) return;
    gecmisDetayGoster(k, 'Sosyal Medya');
}

function gecmisWebDetayGoster(index) {
    var k = yayinGecmisi.webSitesi[index];
    if (!k) return;
    gecmisDetayGoster(k, 'Web Sitesi');
}

function gecmisDetayGoster(k, tip) {
    var overlay = document.getElementById('gecmisModalOverlay');
    var modal = document.getElementById('gecmisModal');
    var baslikEl = document.getElementById('gecmisModalBaslik');
    var icerikEl = document.getElementById('gecmisModalIcerik');

    if (!overlay || !modal || !baslikEl || !icerikEl) return;

    baslikEl.textContent = tip + ' Yayin Detayi';

    var html = '';

    html += '<div class="detay-satir"><span class="detay-etiket">Yayin Tarihi ve Saati</span><span class="detay-deger">' + (k.tarihSaat || '-') + '</span></div>';

    if (tip === 'Sosyal Medya') {
        html += '<div class="detay-satir"><span class="detay-etiket">Paylasim Turu</span><span class="detay-deger">' + (k.tur || '-') + '</span></div>';
        html += '<div class="detay-satir"><span class="detay-etiket">Sira Numarasi</span><span class="detay-deger">' + (k.siraNumarasi || '-') + '</span></div>';
    } else {
        html += '<div class="detay-satir"><span class="detay-etiket">Web Bolumu</span><span class="detay-deger">' + (k.webBolum || '-') + '</span></div>';
        if (k.icerikId) {
            html += '<div class="detay-satir"><span class="detay-etiket">Icerik ID</span><span class="detay-deger">' + k.icerikId + '</span></div>';
        }
        if (k.icerikAdres) {
            html += '<div class="detay-satir"><span class="detay-etiket">Icerik Adresi</span><span class="detay-deger"><a href="' + k.icerikAdres + '" target="_blank" rel="noopener">' + k.icerikAdres + '</a></span></div>';
        }
    }

    html += '<div class="detay-satir"><span class="detay-etiket">Baslik</span><span class="detay-deger">' + (k.baslik || '-') + '</span></div>';
    html += '<div class="detay-satir"><span class="detay-etiket">Gorsel / Icerik Adi</span><span class="detay-deger">' + (k.gorselAdi || '-') + '</span></div>';

    if (tip === 'Sosyal Medya') {
        html += '<div class="detay-satir"><span class="detay-etiket">Platform</span><span class="detay-deger">' + (k.platform || '-') + '</span></div>';
        html += '<div class="detay-satir"><span class="detay-etiket">Sablon Bilgisi</span><span class="detay-deger">' + (k.sablon || '-') + '</span></div>';
        html += '<div class="detay-satir"><span class="detay-etiket">Platforma Ozel Cikti</span><span class="detay-deger">' + (k.platformCikti || '-') + '</span></div>';
        if (k.durum === 'basarili') {
            html += '<div class="detay-satir"><span class="detay-etiket">Yayin Baglantisi</span><span class="detay-deger">' + (k.baglanti || 'Baglanti henuz kullanima hazir degildir. Gercek entegrasyon teknik sartnamede tanimlanacaktir.') + '</span></div>';
        }
    }

    html += '<div class="detay-satir"><span class="detay-etiket">Yayin Durumu</span><span class="detay-deger">' + (k.durum === 'basarili' ? 'Basarili' : (k.durum === 'basarisiz' ? 'Basarisiz' : 'Bekliyor')) + '</span></div>';

    if (k.durum === 'basarisiz') {
        html += '<div class="detay-satir"><span class="detay-etiket">Hata Nedeni</span><span class="detay-deger">' + (k.hataNedeni || 'Bilinmeyen bir hata olustu.') + '</span></div>';
    }

    if (k.icerik) {
        html += '<div class="detay-icerik">' + k.icerik + '</div>';
    }

    icerikEl.innerHTML = html;
    overlay.style.display = 'block';
    modal.style.display = 'flex';
}

function gecmisModalKapat() {
    var overlay = document.getElementById('gecmisModalOverlay');
    var modal = document.getElementById('gecmisModal');
    if (overlay) overlay.style.display = 'none';
    if (modal) modal.style.display = 'none';
}

// ===== FAZ 6 - OTOMATIK YAYIN DONGUSU =====
var otomatikSistem = {
    aktif: true,
    gunlukKota: 5,
    bugunKullanilan: 0,
    standartKayitlar: [],   // { siraNo, baslik, tur, tarih, gorselAdi, videoPath, mesaj, mediaList }
    kampanyaKayitlar: [],   // { baslik, baslangic, bitis, tur, tarih, gorselAdi, videoPath, mesaj, mediaList }
    detayliKayitlar: [],    // { baslik, mesaj, gorselAdi, videoPath, mediaList, planTarihSaat, durum, tur }
    sonCalisma: null,
    sonYayinZamani: null,
    yayinCalisiyor: false
};

// ---- KALICILIK: yerel mimaride uygulama kapanisinda veri kaybi onlenir ----
function kaliciKaydet(anahtar, deger) {
    try { localStorage.setItem(anahtar, JSON.stringify(deger)); } catch (e) {}
}

function kaliciOku(anahtar, varsayilan) {
    try {
        var s = localStorage.getItem(anahtar);
        return s ? JSON.parse(s) : varsayilan;
    } catch (e) {
        return varsayilan;
    }
}

function gecmisiKaydet() {
    kaliciKaydet('es_ops_yayin_gecmisi', yayinGecmisi);
}

function gecmisiYukle() {
    var d = kaliciOku('es_ops_yayin_gecmisi', null);
    if (d && d.sosyalMedya && d.webSitesi) {
        yayinGecmisi = { sosyalMedya: d.sosyalMedya, webSitesi: d.webSitesi };
    }
}

function otomatikSistemiKaydet() {
    kaliciKaydet('es_ops_otomatik_sistem', {
        aktif: otomatikSistem.aktif,
        bugunKullanilan: otomatikSistem.bugunKullanilan,
        standartKayitlar: otomatikSistem.standartKayitlar,
        kampanyaKayitlar: otomatikSistem.kampanyaKayitlar,
        detayliKayitlar: otomatikSistem.detayliKayitlar,
        sonCalisma: otomatikSistem.sonCalisma,
        sonYayinZamani: otomatikSistem.sonYayinZamani
    });
}

function otomatikSistemiYukle() {
    var d = kaliciOku('es_ops_otomatik_sistem', null);
    if (!d) return;
    if (typeof d.aktif === 'boolean') otomatikSistem.aktif = d.aktif;
    if (typeof d.bugunKullanilan === 'number') otomatikSistem.bugunKullanilan = d.bugunKullanilan;
    if (Array.isArray(d.standartKayitlar)) otomatikSistem.standartKayitlar = d.standartKayitlar;
    if (Array.isArray(d.kampanyaKayitlar)) otomatikSistem.kampanyaKayitlar = d.kampanyaKayitlar;
    if (Array.isArray(d.detayliKayitlar)) otomatikSistem.detayliKayitlar = d.detayliKayitlar;
    if (d.sonCalisma) otomatikSistem.sonCalisma = d.sonCalisma;
    if (d.sonYayinZamani) otomatikSistem.sonYayinZamani = d.sonYayinZamani;
}

function bildirimleriKaydet() {
    kaliciKaydet('es_ops_bildirimler', bildirimler);
}

function bildirimleriYukle() {
    var d = kaliciOku('es_ops_bildirimler', null);
    if (Array.isArray(d)) bildirimler = d;
}

// ---- Planlı yayınların ve otomatik döngünün zamanlayıcısı ----
var zamanlayiciId = null;
var OTOMATIK_YAYIN_ARALIgi_MS = 5 * 60 * 1000; // en az 5 dakika aralik

function otomatikBilgilendir(sessiz, msg) {
    if (!sessiz) alert(msg);
}

function planliBekleyenVar() {
    return (otomatikSistem.detayliKayitlar || []).some(function (k) {
        return k.durum === 'bekliyor';
    });
}

function zamanlayiciBaslat() {
    if (zamanlayiciId !== null) return;
    zamanlayiciId = setInterval(function () {
        planliYayinTetikle();
        otomatikDonguTetikle();
    }, 30000);
}

// Planlı (Detaylı Paylasim) kayitlarin vakt en gelip gelmedigini denetler.
function planliYayinTetikle() {
    if (!otomatikSistem.detayliKayitlar) return;
    var simdiMs = Date.now();
    otomatikSistem.detayliKayitlar.forEach(function (k) {
        if (!k || k.durum !== 'bekliyor') return;
        var planMs = new Date(k.planTarihSaat).getTime();
        if (isNaN(planMs) || planMs > simdiMs) return;
        k.durum = 'yayinlaniyor';
        otomatikSistemiKaydet();
        var icerik = planliKayitIcerik(k);
        sosyalGercekYayinGonder(icerik).then(
            function (bilgi) {
                planliOnayKaydet(k, bilgi);
            },
            function () {
                planliOnayKaydet(k, { platformlar: [], toplamBasarili: 0, toplamBasarisiz: 0, bagliHesapYok: true });
            }
        );
    });
}

function planliOnayKaydet(k, bilgi) {
    var basarili = bilgi.toplamBasarili > 0;
    k.durum = basarili ? 'yayinlandi' : 'basarisiz';
    var platformlar = (bilgi.platformlar || []).map(function (r) { return r.platformId; }).join(', ');
    gecmisSMKayitEkle({
        tarihSaat: new Date().toLocaleString('tr-TR'),
        tur: (k.tur || 'Detayli Paylasim') + ' (Planli)',
        siraNumarasi: '',
        baslik: k.baslik || 'Planli paylasım',
        gorselAdi: k.gorselAdi || 'Medya',
        platform: platformlar || 'Aktif platformlar',
        sablon: 'Standart',
        platformCikti: basarili ? 'Yayin ID: ' + (bilgi.platformlar.map(function (r) { return r.postId || ''; }).filter(Boolean).join(', ') || 'basarili') : 'Yayin basarisiz',
        durum: basarili ? 'basarili' : 'basarisiz',
        icerik: k.mesaj || k.baslik || '',
        baglanti: basarili ? 'Planli yayin gerceklestirildi.' : '',
        hataNedeni: basarili ? '' : (bilgi.platformlar.map(function (r) { return r.hataMesaji || 'Yayin basarisiz'; }).join('; '))
    });
    if (basarili) {
        bildirimEkle('sosyal-medya-baglanti', 'basarili', 'Planli yayin basarili', (k.baslik || '') + ' planli yayini gerceklestirildi.');
    } else {
        bildirimEkle('yayin-hatasi', 'hata', 'Planli yayin basarisiz', (k.baslik || '') + ' yayinlanamadi.');
    }
    otomatikSistemiKaydet();
}

function planliKayitIcerik(k) {
    return {
        mesaj: k.mesaj || k.baslik || 'Planli paylasim',
        baslik: k.baslik || 'Planli paylasim',
        medyaVar: k.mediaList && k.mediaList.length > 0,
        mediaKind: k.mediaKind || ((k.mediaList && k.mediaList.length) ? (k.videoPath ? 'video' : 'image') : ''),
        mediaFiles: k.mediaList || [],
        mediaUrls: [],
        videoPath: k.videoPath || '',
        privacyLevel: 'SELF_ONLY'
    };
}

// Otomatik döngü: sistem aktifken kotaya takilmayan ve süre geçtiyse bir kayıt yayınlar.
function otomatikDonguTetikle() {
    if (!otomatikSistem.aktif) return;
    if (otomatikSistem.bugunKullanilan >= otomatikSistem.gunlukKota) return;
    if (otomatikSistem.yayinCalisiyor) return;
    if (otomatikSistem.sonYayinZamani) {
        var fark = Date.now() - new Date(otomatikSistem.sonYayinZamani).getTime();
        if (!isNaN(fark) && fark < OTOMATIK_YAYIN_ARALIK_MS) return;
    }
    if (otomatikSistem.standartKayitlar.length === 0 && !otomatikAktifKampanyaVar() && !planliBekleyenVar()) return;
    otomatikSimuleEt(true);
}

// Gunluk sifirlama kontrolu - sayfa acildiginda gun degisti mi kontrol et
function otomatikGunlukKontrol() {
    var bugun = new Date().toDateString();
    var kayitli = localStorage.getItem('otomatik_gun');
    if (kayitli !== bugun) {
        otomatikSistem.bugunKullanilan = 0;
        localStorage.setItem('otomatik_gun', bugun);
        localStorage.setItem('otomatik_kota', '0');
    } else {
        var kotali = localStorage.getItem('otomatik_kota');
        otomatikSistem.bugunKullanilan = kotali ? parseInt(kotali, 10) : 0;
    }
}

function otomatikKotaKaydet() {
    localStorage.setItem('otomatik_kota', otomatikSistem.bugunKullanilan.toString());
}

function otomatikDurumuGuncelle() {
    var kart = document.getElementById('otomatikDurumKart');
    if (!kart) return;

    var kalan = otomatikSistem.aktif ? (otomatikSistem.gunlukKota - otomatikSistem.bugunKullanilan) : 0;
    var durumText = '';
    var durumClass = '';

    if (!otomatikSistem.aktif) {
        durumText = 'Duraklatildi';
        durumClass = 'warning';
    } else if (otomatikSistem.bugunKullanilan >= otomatikSistem.gunlukKota) {
        durumText = 'Gunluk kota doldu';
        durumClass = 'danger';
    } else if (otomatikSistem.standartKayitlar.length === 0 && !otomatikAktifKampanyaVar()) {
        durumText = 'Yayinlanacak uygun kayit yok';
        durumClass = 'warning';
    } else {
        durumText = 'Aktif';
        durumClass = 'success';
    }

    var siradaki = otomatikSiradakiBul();
    var siradakiText = siradaki ? (siradaki.baslik || '(baslik yok)') : 'Yok';

    var aktifKampanyaSayisi = 0;
    otomatikSistem.kampanyaKayitlar.forEach(function(k) {
        var now = new Date();
        var bas = new Date(k.baslangic);
        var bit = new Date(k.bitis);
        if (now >= bas && now <= bit) { aktifKampanyaSayisi++; }
    });

    var html = '<div class="otomatik-durum">';
    html += '<div class="durum-satir"><span class="durum-label">Bugun Kullanilan</span><span class="durum-value">' + otomatikSistem.bugunKullanilan + ' / ' + otomatikSistem.gunlukKota + '</span></div>';
    html += '<div class="durum-satir"><span class="durum-label">Kalan Hak</span><span class="durum-value ' + (kalan <= 1 ? 'danger' : '') + '">' + kalan + '</span></div>';
    html += '<div class="durum-satir"><span class="durum-label">Siradaki Yayin</span><span class="durum-value">' + siradakiText + '</span></div>';
    html += '<div class="durum-satir"><span class="durum-label">Bekleyen Kayit</span><span class="durum-value">' + otomatikSistem.standartKayitlar.length + '</span></div>';
    html += '<div class="durum-satir"><span class="durum-label">Aktif Kampanya</span><span class="durum-value">' + aktifKampanyaSayisi + '</span></div>';
    html += '<div class="durum-satir"><span class="durum-label">Sistem Durumu</span><span class="durum-value ' + durumClass + '">' + durumText + '</span></div>';
    html += '</div>';
    kart.innerHTML = html;

    var yonetimKart = document.getElementById('otomatikYonetimKart');
    if (yonetimKart) {
        var yHtml = '<div class="otomatik-yonetim">';
        if (otomatikSistem.aktif) {
            yHtml += '<button class="yonetim-btn pasif" onclick="otomatikDuraklat()">Sistemi Duraklat</button>';
        } else {
            yHtml += '<button class="yonetim-btn aktif" onclick="otomatikDevamEttir()">Sistemi Devam Ettir</button>';
        }
        yHtml += '<button class="yonetim-btn bilgi" onclick="otomatikSimuleEt()">Otomatik Yayini Simule Et</button>';
        yHtml += '<div style="font-size:0.78rem;color:#9ca3af;margin-top:8px;">Otomatik yayin; bagli hesap varsa gercek yayin motoruyla paylasilir, yoksa kayit sirada korunur ve kotadan dusulmez.</div>';
        yHtml += '</div>';
        yonetimKart.innerHTML = yHtml;
    }
}

function otomatikAktifKampanyaVar() {
    var now = new Date();
    for (var i = 0; i < otomatikSistem.kampanyaKayitlar.length; i++) {
        var k = otomatikSistem.kampanyaKayitlar[i];
        var bas = new Date(k.baslangic);
        var bit = new Date(k.bitis);
        if (now >= bas && now <= bit) return true;
    }
    return false;
}

function otomatikSiradakiBul() {
    // Standart paylasimlar sirasina gore
    if (otomatikSistem.standartKayitlar.length > 0) {
        otomatikSistem.standartKayitlar.sort(function(a, b) {
            return (parseInt(a.siraNo, 10) || 0) - (parseInt(b.siraNo, 10) || 0);
        });
        return otomatikSistem.standartKayitlar[0];
    }
    // Kampanyalar kendi baslangic tarihlerine gore
    var now = new Date();
    var uygunKampanya = null;
    otomatikSistem.kampanyaKayitlar.forEach(function(k) {
        var bas = new Date(k.baslangic);
        var bit = new Date(k.bitis);
        if (now >= bas && now <= bit) {
            if (!uygunKampanya || new Date(k.baslangic) < new Date(uygunKampanya.baslangic)) {
                uygunKampanya = k;
            }
        }
    });
    return uygunKampanya;
}

function otomatikDuraklat() {
    otomatikSistem.aktif = false;
    alert('Otomatik yayin sistemi duraklatildi. Mevcut kayitlar, sira numaralari ve kampanyalar korunuyor.');
    otomatikDurumuGuncelle();
}

function otomatikDevamEttir() {
    otomatikSistem.aktif = true;
    alert('Otomatik yayin sistemi devam ettiriliyor. Mevcut sira ve kampanya durumlari korunuyor.');
    otomatikDurumuGuncelle();
}

function otomatikSimuleEt(sessiz) {
    if (!otomatikSistem.aktif) {
        otomatikBilgilendir(sessiz, 'Otomatik yayin sistemi duraklatilmistir. Once sistemi devam ettirin.');
        return;
    }
    if (otomatikSistem.bugunKullanilan >= otomatikSistem.gunlukKota) {
        otomatikBilgilendir(sessiz, 'Gunluk otomatik paylasim kotasi (' + otomatikSistem.gunlukKota + ') dolmustur.');
        return;
    }
    if (otomatikSistem.yayinCalisiyor) return;

    var simdi = new Date().toLocaleString('tr-TR');
    var yapilanKayit = null;
    var kayitTuru = '';
    var siraNo = '';

    var siraKuyruktur = false;
    if (otomatikSistem.standartKayitlar.length > 0) {
        otomatikSistem.standartKayitlar.sort(function(a, b) {
            return (parseInt(a.siraNo, 10) || 0) - (parseInt(b.siraNo, 10) || 0);
        });
        var kayit = otomatikSistem.standartKayitlar.shift();
        siraKuyruktur = true;
        kayitTuru = 'Standart Paylasim (Otomatik)';
        siraNo = kayit.siraNo;
        yapilanKayit = kayit;
    } else if (otomatikAktifKampanyaVar()) {
        var now = new Date();
        for (var i = 0; i < otomatikSistem.kampanyaKayitlar.length; i++) {
            var k = otomatikSistem.kampanyaKayitlar[i];
            var bas = new Date(k.baslangic);
            var bit = new Date(k.bitis);
            if (now >= bas && now <= bit) {
                kayitTuru = 'Kampanya (Otomatik)';
                siraNo = '';
                yapilanKayit = k;
                break;
            }
        }
    }

    if (!yapilanKayit) {
        otomatikBilgilendir(sessiz, 'Yayinlanacak uygun kayit bulunamadi. (Siradaki standart paylasim yok, aktif kampanya yok)');
        otomatikDurumuGuncelle();
        return;
    }

    var mediaList = yapilanKayit.mediaList || [];
    if (!Array.isArray(mediaList)) mediaList = [];
    var videoYolu = yapilanKayit.videoPath || (mediaList.length ? mediaList[mediaList.length - 1] : '');
    if (typeof videoYolu !== 'string') videoYolu = '';
    var gorselAdi = yapilanKayit.gorselAdi || (mediaList.length ? medyaYolAdi(mediaList[0]) : 'Medya');

    var icerik = {
        mesaj: yapilanKayit.mesaj || yapilanKayit.baslik || 'Otomatik paylasim',
        baslik: yapilanKayit.baslik || 'Otomatik paylasim',
        medyaVar: mediaList.length > 0,
        mediaKind: mediaList.length ? (videoYolu ? 'video' : 'image') : '',
        mediaFiles: mediaList,
        mediaUrls: [],
        videoPath: videoYolu,
        privacyLevel: 'SELF_ONLY'
    };

    otomatikSistem.yayinCalisiyor = true;
    otomatikSistemiKaydet();

    sosyalGercekYayinGonder(icerik).then(function (bilgi) {
        otomatikSistem.yayinCalisiyor = false;
        bilgi = bilgi || { platformlar: [], toplamBasarili: 0, toplamBasarisiz: 0, bagliHesapYok: true };
        var platformlar = bilgi.platformlar || [];
        var basariliToplam = bilgi.toplamBasarili || 0;
        var bagliYok = !!bilgi.bagliHesapYok;

        var platformOzet = platformlar.map(function (r) { return r.platformId + (r.basarili ? ' (OK)' : ' (HATA)'); }).join(' | ');
        var hataMesaji = platformlar.filter(function (r) { return !r.basarili; }).map(function (r) { return r.hataMesaji || r.error || 'Yayin basarisiz'; }).join('; ');

        if (bagliYok) {
            if (siraKuyruktur) otomatikSistem.standartKayitlar.push(yapilanKayit);
            gecmisSMKayitEkle({
                tarihSaat: simdi,
                tur: kayitTuru,
                siraNumarasi: siraNo,
                baslik: yapilanKayit.baslik || '(baslik yok)',
                gorselAdi: gorselAdi,
                platform: 'Aktif platformlar',
                sablon: 'Standart',
                platformCikti: 'Bagli hesap yok',
                durum: 'basarisiz',
                icerik: 'Otomatik yayin yapilamadi: bagli hesap bulunamadi.',
                baglanti: '',
                hataNedeni: 'Bagli sosyal medya hesabi yok. Kayit sirasinda korundu, kotadan dusulmedi.',
                otomatik: true
            });
            otomatikSistem.sonYayinZamani = new Date().toISOString();
            otomatikSistemiKaydet();
            otomatikBilgilendir(sessiz, 'Otomatik yayin yapilamadi: bagli hesap bulunamadi. Kotadan dusulmedi.');
            otomatikDurumuGuncelle();
            return;
        }

        var basarili = basariliToplam > 0;
        if (siraKuyruktur) otomatikSistem.standartKayitlar.push(yapilanKayit);
        if (basarili) {
            otomatikSistem.bugunKullanilan++;
            otomatikKotaKaydet();
            otomatikSistem.sonYayinZamani = new Date().toISOString();
        }
        if (!basarili) otomatikSistem.sonYayinZamani = new Date().toISOString();
        otomatikSistemiKaydet();
        gecmisSMKayitEkle({
            tarihSaat: simdi,
            tur: kayitTuru,
            siraNumarasi: siraNo,
            baslik: yapilanKayit.baslik || '(baslik yok)',
            gorselAdi: gorselAdi,
            platform: platformlar.map(function (r) { return r.platformId; }).join(', ') || 'Gercek yayin motoru',
            sablon: 'Standart',
            platformCikti: basarili ? (platformlar.map(function (r) { return r.postId || r.platformId; }).join(', ') || 'Yayin ID: basarili') : (bagliYok ? 'Bagli hesap yok' : (platformOzet || 'Yayin basarisiz')),
            durum: basarili ? 'basarili' : 'basarisiz',
            icerik: yapilanKayit.mesaj || 'Otomatik yayin',
            baglanti: basarili ? ('Kota: ' + otomatikSistem.bugunKullanilan + '/' + otomatikSistem.gunlukKota) : '',
            hataNedeni: basarili ? '' : hataMesaji,
            otomatik: true
        });
        if (basarili) {
            bildirimEkle('sosyal-medya-baglanti', 'basarili', 'Otomatik yayin basarili', 'Otomatik yayin gerceklestirildi. Kota: ' + otomatikSistem.bugunKullanilan + '/' + otomatikSistem.gunlukKota);
            otomatikBilgilendir(sessiz, 'Otomatik yayın basarili. (' + platformOzet + ') Kota: ' + otomatikSistem.bugunKullanilan + '/' + otomatikSistem.gunlukKota);
        } else {
            bildirimEkle('yayin-hatasi', 'hata', 'Otomatik yayın basarisiz', hataMesaji || 'Yayin basarisiz');
            otomatikBilgilendir(sessiz, (platformlar.length ? platformlar.length + ' platformda' : '') + ' otomatik yayin basarisiz. Kotadan dusulmedi.');
        }
        otomatikDurumuGuncelle();
    }, function (err) {
        otomatikSistem.yayinCalisiyor = false;
        var msg = metaHataMesaji(String((err && (err.message || err.code || err)) || ''));
        if (siraKuyruktur) otomatikSistem.standartKayitlar.push(yapilanKayit);
        otomatikSistem.sonYayinZamani = new Date().toISOString();
        otomatikSistemiKaydet();
        gecmisSMKayitEkle({
            tarihSaat: simdi,
            tur: kayitTuru,
            siraNumarasi: siraNo,
            baslik: yapilanKayit.baslik || '(baslik yok)',
            gorselAdi: gorselAdi,
            platform: 'Gercek yayin motoru',
            sablon: 'Standart',
            platformCikti: 'Yayin istendi reddedildi',
            durum: 'basarisiz',
            icerik: 'Otomatik yayin basarisiz.',
            baglanti: '',
            hataNedeni: msg,
            otomatik: true
        });
        bildirimEkle('yayin-hatasi', 'hata', 'Otomatik yayın basarisiz', msg);
        otomatikBilgilendir(sessiz, 'Otomatik yayın basarisiz. Kayıt sırasında korundu, kotadan dusulmedi.');
        otomatikDurumuGuncelle();
    });
}

// Standart paylasim kaydedildiginde otomatik siraya ekle
function otomatikStandartEkle(baslik, siraNo, gorselAdi, videoYolu, mesaj, mediaList) {
    otomatikSistem.standartKayitlar.push({
        siraNo: siraNo,
        baslik: baslik,
        tur: 'Standart Paylasim',
        tarih: new Date().toISOString(),
        gorselAdi: gorselAdi || 'Gorsel (simule)',
        videoPath: videoYolu || '',
        mesaj: mesaj || '',
        mediaList: Array.isArray(mediaList) ? mediaList : []
    });
    otomatikDurumuGuncelle();
}

// Kampanya kaydedildiginde
function otomatikKampanyaEkle(baslik, baslangic, bitis, gorselAdi, videoYolu, mesaj, mediaList) {
    otomatikSistem.kampanyaKayitlar.push({
        baslik: baslik,
        baslangic: baslangic,
        bitis: bitis,
        tur: 'Kampanya',
        tarih: new Date().toISOString(),
        gorselAdi: gorselAdi || 'Gorsel (simule)',
        videoPath: videoYolu || '',
        mesaj: mesaj || '',
        mediaList: Array.isArray(mediaList) ? mediaList : []
    });
    otomatikDurumuGuncelle();
}

// Detayli (planli) paylasim kaydedildiginde: zamanlayici tarih/saatte yayinlar.
function otomatikDetayliEkle(baslik, mesaj, gorselAdi, videoYolu, mediaList, tarih, saat) {
    var planMs = Date.now();
    if (tarih && saat) {
        var p = new Date(tarih + 'T' + saat + ':00');
        if (!isNaN(p.getTime())) planMs = p.getTime();
    }
    otomatikSistem.detayliKayitlar.push({
        baslik: baslik || 'Planli paylasim',
        mesaj: mesaj || '',
        tur: 'Detayli Paylasim',
        tarih: new Date().toISOString(),
        gorselAdi: gorselAdi || 'Medya',
        videoPath: videoYolu || '',
        mediaList: Array.isArray(mediaList) ? mediaList : [],
        planTarihSaat: new Date(planMs).toISOString(),
        durum: 'bekliyor'
    });
    otomatikSistemiKaydet();
    otomatikDurumuGuncelle();
}

// Sayfa yuklendiginde
document.addEventListener('DOMContentLoaded', function() {
    gecmisiYukle();
    bildirimleriYukle();
    otomatikGunlukKontrol();
    otomatikSistemiYukle();
    otomatikDurumuGuncelle();
    zamanlayiciBaslat();
    planliYayinTetikle();
    otomatikDonguTetikle();
});


// ===== FAZ 7 - BILDIRIM MERKEZI =====
var bildirimler = [];
var bildirimSeciliKategori = 'tumu';

var bildirimKategorileri = [
    'sistem-uyari', 'sosyal-medya-baglanti', 'web-baglanti',
    'yayin-hatasi', 'kampanya', 'lisans', 'genel'
];

var bildirimTurleri = ['uyari', 'hata', 'bilgi', 'basarili'];

// Dashboard bildirim ozetini guncelle
function bildirimDashboardOzetGuncelle() {
    // Bildirim Ozeti kartini bul - dashboard-grid icinde "Bildirim Özeti" baslikli karti bul
    var ozetEl = null;
    var kartlar = document.querySelectorAll('#dash-sosyal-medya .dashboard-grid .dashboard-card');
    for (var i = 0; i < kartlar.length; i++) {
        var titleEl = kartlar[i].querySelector('.card-title');
        if (titleEl && titleEl.textContent.trim() === 'Bildirim Özeti') {
            ozetEl = kartlar[i].querySelector('.card-placeholder');
            break;
        }
    }
    if (!ozetEl) return;
    
    var okunmamis = bildirimler.filter(function(b) { return !b.okundu; }).length;
    var toplam = bildirimler.length;
    
    if (toplam === 0) {
        ozetEl.textContent = 'Henüz bildirim bulunmuyor.';
    } else {
        ozetEl.textContent = okunmamis + ' okunmamis, ' + toplam + ' toplam bildirim';
    }
}

function bildirimEkle(kategori, tur, baslik, aciklama) {
    if (bildirimKategorileri.indexOf(kategori) === -1) return;
    if (bildirimTurleri.indexOf(tur) === -1) return;

    var bildirim = {
        id: Date.now() + Math.random(),
        kategori: kategori,
        tur: tur,
        baslik: baslik,
        aciklama: aciklama,
        tarih: new Date().toLocaleDateString('tr-TR'),
        saat: new Date().toLocaleTimeString('tr-TR', { hour: '2-digit', minute: '2-digit' }),
        okundu: false
    };

    bildirimler.unshift(bildirim);
    bildirimListele();
    bildirimSayaciGuncelle();
    bildirimDashboardOzetGuncelle();
    bildirimleriKaydet();
}

function bildirimSayaciGuncelle() {
    var okunmamis = 0;
    var i;
    for (i = 0; i < bildirimler.length; i++) {
        if (!bildirimler[i].okundu) okunmamis++;
    }

    var sayacEl = document.getElementById('bildirimOkunmamisSayisi');
    if (sayacEl) sayacEl.textContent = okunmamis;

    // Sidebar badge
    var badge = document.getElementById('sidebarBildirimBadge');
    if (badge) {
        if (okunmamis > 0) {
            badge.textContent = okunmamis > 99 ? '99+' : okunmamis;
            badge.style.display = 'inline-block';
        } else {
            badge.style.display = 'none';
        }
    }
}

function bildirimFiltrele(kategori) {
    bildirimSeciliKategori = kategori;

    document.querySelectorAll('.kategori-tab').forEach(function(el) {
        el.classList.remove('active');
        if (el.getAttribute('data-kategori') === kategori) {
            el.classList.add('active');
        }
    });

    bildirimListele();
}

function bildirimListele() {
    var liste = document.getElementById('bildirimListe');
    if (!liste) return;

    var filtrelenmis = bildirimler;
    if (bildirimSeciliKategori !== 'tumu') {
        filtrelenmis = bildirimler.filter(function(b) {
            return b.kategori === bildirimSeciliKategori;
        });
    }

    if (filtrelenmis.length === 0) {
        if (bildirimler.length === 0) {
            liste.innerHTML = '<div class="bildirim-bos">Henuz bildirim bulunmuyor.</div>';
        } else {
            liste.innerHTML = '<div class="bildirim-bos">Bu kategoride bildirim bulunmuyor.</div>';
        }
        return;
    }

    var html = '';
    filtrelenmis.forEach(function(b) {
        var okunmadiClass = b.okundu ? '' : ' okunmadi';
        var turText = '';
        var turIcon = '';

        switch (b.tur) {
            case 'uyari': turIcon = '!'; turText = 'Uyari'; break;
            case 'hata': turIcon = 'X'; turText = 'Hata'; break;
            case 'bilgi': turIcon = 'i'; turText = 'Bilgi'; break;
            case 'basarili': turIcon = 'V'; turText = 'Basarili'; break;
        }

        var kategoriAdlari = {
            'sistem-uyari': 'Sistem Uyarisi',
            'sosyal-medya-baglanti': 'Sosyal Medya Baglanti',
            'web-baglanti': 'Web Sitesi Baglanti',
            'yayin-hatasi': 'Yayin Hatasi',
            'kampanya': 'Kampanya Uyarisi',
            'lisans': 'Lisans Uyarisi',
            'genel': 'Genel Bilgilendirme'
        };
        var kategoriAdi = kategoriAdlari[b.kategori] || b.kategori;

        html += '<div class="bildirim-kart' + okunmadiClass + '">';
        html += '    <div class="bildirim-tur-icon ' + b.tur + '">' + turIcon + '</div>';
        html += '    <div class="bildirim-icerik">';
        html += '        <div class="bildirim-baslik">' + b.baslik + '</div>';
        if (b.aciklama) {
            html += '        <div class="bildirim-aciklama">' + b.aciklama + '</div>';
        }
        html += '        <div class="bildirim-meta">';
        html += '            <span>' + b.tarih + ' ' + b.saat + '</span>';
        html += '            <span class="bildirim-kategori-etiketi">' + kategoriAdi + '</span>';
        html += '        </div>';
        html += '    </div>';
        html += '    <div class="bildirim-okundu-action">';
        if (b.okundu) {
            html += '        <button onclick="bildirimOkunmadiYap(' + bildirimler.indexOf(b) + ')">Okunmadi Isaretle</button>';
        } else {
            html += '        <button onclick="bildirimOkunduYap(' + bildirimler.indexOf(b) + ')">Okundu Isaretle</button>';
        }
        html += '    </div>';
        html += '</div>';
    });

    liste.innerHTML = html;
}

function bildirimOkunduYap(index) {
    if (bildirimler[index]) {
        bildirimler[index].okundu = true;
        bildirimListele();
        bildirimSayaciGuncelle();
        bildirimDashboardOzetGuncelle();
    }
}

function bildirimOkunmadiYap(index) {
    if (bildirimler[index]) {
        bildirimler[index].okundu = false;
        bildirimListele();
        bildirimSayaciGuncelle();
        bildirimDashboardOzetGuncelle();
    }
}

function bildirimTumunuOkunduYap() {
    bildirimler.forEach(function(b) {
        b.okundu = true;
    });
    bildirimListele();
    bildirimSayaciGuncelle();
    bildirimDashboardOzetGuncelle();
}

// Mevcut sistem olaylarindan bildirim olusturma
// simulateNow - baglanti durumunu gercek sonuca gore yansitiyor
// (gercek yayin sonuclari simulateNow icindeki per-platform islemiyle gecmise
// islenir ve bildirimler orada olusturulur; burada ek sahte bildirim uretilmez).

// simulateSave bildirim
(function() {
    var originalSimulateSave = simulateSave;
    simulateSave = function(type) {
        originalSimulateSave(type);
        var names = { 'standart': 'Standart', 'kampanya': 'Kampanya', 'detayli': 'Detayli' };
        bildirimEkle('sistem-uyari', 'bilgi',
            names[type] + ' paylasim kaydedildi',
            names[type] + ' paylasim basariyla kaydedildi ve otomatik yayin sirasina eklendi.'
        );
    };
})();

// Web sitesi entegrasyon yok bildirimi
(function() {
    var originalWebYayinla = webYayinla;
    webYayinla = function(icerik, baslik, bolum) {
        originalWebYayinla(icerik, baslik, bolum);
        bildirimEkle('web-baglanti', 'uyari',
            'Web sitesi entegrasyonu bulunmuyor',
            'Web sitesi yayinlama entegrasyonu henuz yapilandirilmamistir. Icerik yayinlanamadi.'
        );
    };
})();

// Otomatik yayin sistemi duraklatildi / devam ettirildi
(function() {
    var originalDuraklat = otomatikDuraklat;
    otomatikDuraklat = function() {
        originalDuraklat();
        bildirimEkle('sistem-uyari', 'uyari',
            'Otomatik yayin sistemi duraklatildi',
            'Otomatik yayin sistemi kullanici tarafindan duraklatilmistir. Yeni otomatik yayin yapilmayacak.'
        );
    };

    var originalDevam = otomatikDevamEttir;
    otomatikDevamEttir = function() {
        originalDevam();
        bildirimEkle('sistem-uyari', 'bilgi',
            'Otomatik yayin sistemi devam ettirildi',
            'Otomatik yayin sistemi kullanici tarafindan tekrar aktif edilmistir.'
        );
    };
})();


// Ilk yuklemede bildirim sayaci
document.addEventListener('DOMContentLoaded', function() {
    bildirimSayaciGuncelle();
});


// ===== FAZ 6 - RAPORLAR =====
function switchRaporTab(tab) {
    document.querySelectorAll('.rapor-tabs .tab').forEach(function(el) {
        el.classList.remove('active');
        if (el.getAttribute('data-rapor-tab') === tab) {
            el.classList.add('active');
        }
    });
    var sm = document.getElementById('rapor-sosyal-medya');
    var web = document.getElementById('rapor-web-sitesi');
    var genel = document.getElementById('rapor-genel');
    if (sm) sm.style.display = (tab === 'sosyal-medya') ? 'block' : 'none';
    if (web) web.style.display = (tab === 'web-sitesi') ? 'block' : 'none';
    if (genel) genel.style.display = (tab === 'genel') ? 'block' : 'none';
    raporlariGuncelle();
}

function raporlariGuncelle() {
    var smKayitlar = yayinGecmisi.sosyalMedya || [];
    var webKayitlar = yayinGecmisi.webSitesi || [];
    
    // SM rapor
    var smToplam = smKayitlar.length;
    var smBasari = smKayitlar.filter(function(k) { return k.durum === 'basarili'; }).length;
    var smBasarisiz = smKayitlar.filter(function(k) { return k.durum === 'basarisiz'; }).length;
    
    var el = document.getElementById('rapor-sm-toplam');
    if (el) el.textContent = smToplam;
    el = document.getElementById('rapor-sm-basari');
    if (el) el.textContent = smBasari;
    el = document.getElementById('rapor-sm-basarisiz');
    if (el) el.textContent = smBasarisiz;
    
    // Platform bazinda
    el = document.getElementById('rapor-sm-platform');
    if (el) {
        var platformlar = {};
        smKayitlar.forEach(function(k) {
            if (k.platform) {
                var pList = k.platform.split(',');
                pList.forEach(function(p) {
                    var pTrim = p.trim();
                    if (pTrim) platformlar[pTrim] = (platformlar[pTrim] || 0) + 1;
                });
            }
        });
        var pKeys = Object.keys(platformlar);
        if (pKeys.length === 0) {
            el.textContent = 'Henuz veri yok.';
        } else {
            var pHtml = '';
            pKeys.forEach(function(p) {
                pHtml += '<div style="display:flex;justify-content:space-between;padding:4px 0;font-size:0.88rem;"><span>' + p + '</span><span style="font-weight:600;">' + platformlar[p] + '</span></div>';
            });
            el.innerHTML = pHtml;
        }
    }
    
    // Web rapor
    var webToplam = webKayitlar.length;
    var webBasari = webKayitlar.filter(function(k) { return k.durum === 'basarili'; }).length;
    var webBasarisiz = webKayitlar.filter(function(k) { return k.durum === 'basarisiz'; }).length;
    
    el = document.getElementById('rapor-web-toplam');
    if (el) el.textContent = webToplam;
    el = document.getElementById('rapor-web-basari');
    if (el) el.textContent = webBasari;
    el = document.getElementById('rapor-web-basarisiz');
    if (el) el.textContent = webBasarisiz;
    
    // Genel rapor
    var genelToplam = smToplam + webToplam;
    el = document.getElementById('rapor-genel-toplam');
    if (el) el.textContent = genelToplam;
    
    el = document.getElementById('rapor-genel-dagilim');
    if (el) el.textContent = 'SM: ' + smToplam + ' / Web: ' + webToplam;
    
    var genelBasari = smBasari + webBasari;
    var oran = genelToplam > 0 ? Math.round((genelBasari / genelToplam) * 100) : 0;
    el = document.getElementById('rapor-genel-oran');
    if (el) el.textContent = oran + '%';
}

// Mevcut gecmis ekleme fonksiyonlarina rapor guncellemesi ekle
(function() {
    var origSM = gecmisSMKayitEkle;
    gecmisSMKayitEkle = function(kayit) {
        origSM(kayit);
        raporlariGuncelle();
    };
    var origWeb = gecmisWebKayitEkle;
    gecmisWebKayitEkle = function(kayit) {
        origWeb(kayit);
        raporlariGuncelle();
    };
})();

// ===== FAZ 8 - AYARLAR =====
var ayarlarPlatformlar = [
    { id: 'instagram', ad: 'Instagram', bagli: false, hesapAdi: '', sonKontrol: '' },
    { id: 'facebook', ad: 'Facebook', bagli: false, hesapAdi: '', sonKontrol: '' },
    { id: 'linkedin', ad: 'LinkedIn', bagli: false, hesapAdi: '', sonKontrol: '' },
    { id: 'x', ad: 'X', bagli: false, hesapAdi: '', sonKontrol: '' },
    { id: 'tiktok', ad: 'TikTok', bagli: false, hesapAdi: '', sonKontrol: '' },
    { id: 'youtube', ad: 'YouTube', bagli: false, hesapAdi: '', sonKontrol: '' }
];

var ayarlarWebBaglanti = {
    bagli: false,
    webAdres: '',
    apiAdres: '',
    authYontem: 'API Key',
    sonKontrol: ''
};

// localStorage anahtarlari
var AYARLAR_STORAGE_KEY = 'es_ayarlar_genel';
var AYARLAR_WEB_KEY = 'es_ayarlar_web';

function switchAyarlarTab(tab) {
    document.querySelectorAll('.ayarlar-tabs .tab').forEach(function(el) {
        el.classList.remove('active');
        if (el.getAttribute('data-ayarlar-tab') === tab) {
            el.classList.add('active');
        }
    });

    var sosyal = document.getElementById('ayarlar-sosyal-medya');
    var web = document.getElementById('ayarlar-web-sitesi');
    var genel = document.getElementById('ayarlar-genel');

    if (sosyal) sosyal.style.display = (tab === 'sosyal-medya') ? 'block' : 'none';
    if (web) web.style.display = (tab === 'web-sitesi') ? 'block' : 'none';
    if (genel) genel.style.display = (tab === 'genel') ? 'block' : 'none';
}

// ===== SOSYAL MEDYA HESAPLARI =====

// Tauri komutlari icin yardimci. Tauri'nin resmi global invoke mekanizmasini
// (window.__TAURI__.core.invoke) kullanir. Mevcut proje withGlobalTauri'yi
// etkinlestirmedigi ve frontend'de bundler/api paketi olmadigi icin, bu global
// nesne yoksa hicbir islem yapilmaz ve null dondurulur. Boylece "Tauri disi
// gelistirme onizlemesi"nde yakalanmamis JavaScript hatasi olusmaz.
function esTauriInvoke(command, args) {
    if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === 'function') {
        return window.__TAURI__.core.invoke(command, args || {});
    }
    return null;
}

// Rust tarafindaki platform katalogundan gelen destek durumu haritasi.
// Teknik support_status degerinin yetkili kaynagi Rust (registry) moduludur;
// bu harita yalniz ondan turetilmis bir onbellektir.
var sosyalKatalog = {};

function sosyalKatalogYukle() {
    var p = esTauriInvoke('social_platform_catalog');
    if (!p) return; // Tauri ortami yok: katalog bos kalir, hata uretilmez
    p.then(function(platforms) {
        if (!platforms) return;
        sosyalKatalog = {};
        platforms.forEach(function(pl) {
            sosyalKatalog[pl.platform_id] = pl;
        });
    }).catch(function() {});
}

function ayarlarPlatformBul(id) {
    for (var i = 0; i < ayarlarPlatformlar.length; i++) {
        if (ayarlarPlatformlar[i].id === id) {
            return ayarlarPlatformlar[i];
        }
    }
    return null;
}

function ayarlarPlatformListele() {
    var liste = document.getElementById('ayarlarPlatformListesi');
    if (!liste) return;

    var html = '';
    ayarlarPlatformlar.forEach(function(p) {
        var durumClass = p.bagli ? 'green' : 'gray';
        var durumText = p.bagli ? 'Bağlı' : 'Bağlı Değil';
        var sonKontrolText = p.sonKontrol ? ('Son kontrol: ' + p.sonKontrol) : '';
        // Facebook/Instagram OAuth öncesi kullanıcı adı girişi gerektirmez;
        // buton etiketi bağlanacak hesabı açıkça belirtir.
        var baglanEtiketi = p.id === 'instagram' ? 'Instagram Hesabını Bağla'
            : p.id === 'facebook' ? 'Facebook Hesabını Bağla' : 'Bağlan';

        html += '<div class="ayarlar-platform-kart">';
        html += '    <div class="platform-ust">';
        html += '        <span class="platform-ad">' + p.ad + '</span>';
        html += '        <span class="platform-durum"><span class="status-dot ' + durumClass + '"></span>' + durumText + '</span>';
        html += '    </div>';
        html += '    <div class="platform-alt">';
        // Tüm platformlar OAuth ile bağlanır; bağlantı öncesi kullanıcı adı
        // veya teknik uygulama kimliği girişi gerekmez (bu değerler hiçbir
        // bağlantı komutuna gönderilmez). OAuth başarıyla tamamlandığında
        // platformdan dönen gerçek hesap adı burada gösterilir.
        if (p.bagli && p.hesapAdi) {
            html += '        <div class="platform-hesap-bilgi">Hesap: ' + p.hesapAdi + '</div>';
        }
        if (sonKontrolText) {
            html += '        <span class="platform-son-kontrol">' + sonKontrolText + '</span>';
        }
        html += '    </div>';
        html += '    <div class="platform-actions">';
        if (!p.bagli) {
            html += '        <button class="btn btn-primary btn-small" onclick="ayarlarPlatformBaglan(\'' + p.id + '\')">' + baglanEtiketi + '</button>';
            html += '        <button class="btn btn-warning btn-small" disabled style="opacity:0.4;cursor:not-allowed;">Bağlantıyı Kes</button>';
        } else {
            html += '        <button class="btn btn-primary btn-small" disabled style="opacity:0.4;cursor:not-allowed;">' + baglanEtiketi + '</button>';
            html += '        <button class="btn btn-warning btn-small" onclick="ayarlarPlatformKes(\'' + p.id + '\')">Bağlantıyı Kes</button>';
        }
        html += '    </div>';
        html += '</div>';
    });

    liste.innerHTML = html;
}

// ===== KONTROLLU HATA KODU -> TURKCE MESAJ =====
// Rust tarafindan dondurulen kisa teknik kodlari, gizli bilgi (token/kod)
// icermeyen anlasilir Turkce bildirime esler. Bilinmeyen kod genel bir mesaja
// duser; asla ham teknik cevap veya token gosterilmez.
function metaHataMesaji(code) {
    var c = String(code || '');
    if (c.indexOf('meta_not_configured') !== -1) {
        return 'Bu surumde Meta uygulama kimligi (App ID) yapilandirilmamis. Bağlantı için güncel sürüm gereklidir; destek ekibiyle iletişime geçin.';
    }
    if (c.indexOf('app_secret_required') !== -1) {
        return 'Bu surumde Meta uygulama gizli anahtari (App Secret) yapilandirilmamis. Bağlantı için güncel sürüm gereklidir; destek ekibiyle iletişime geçin.';
    }
    if (c.indexOf('tiktok_not_configured') !== -1) {
        return 'Bu surumde TikTok uygulama kimlikleri yapilandirilmamis. Bağlantı için güncel sürüm gereklidir; destek ekibiyle iletişime geçin.';
    }
    if (c.indexOf('linkedin_not_configured') !== -1) {
        return 'Bu surumde LinkedIn uygulama kimligi yapilandirilmamis. Bağlantı için güncel sürüm gereklidir; destek ekibiyle iletişime geçin.';
    }
    if (c.indexOf('linkedin_identity_lookup_failed') !== -1) {
        return 'LinkedIn kullanici kimligi alinamadi. Yetkilendirme yari kalabilir; hesabi yeniden baglamayi deneyin.';
    }
    if (c.indexOf('linkedin_org_not_found') !== -1) {
        return 'Yayin yapilabilir (ADMINISTRATOR / CONTENT_ADMIN / DIRECT_SPONSORED_CONTENT_POSTER rolune sahip) yonetilen sirket sayfasi bulunamadi.';
    }
    if (c.indexOf('reauthorization_required') !== -1) {
        return 'Token yenileme app secret gerektirdigi icin yapilamadi. Do\u011fru yetkilendirme icin hesabin yeniden baglanmasi gerekir (bu surumde engellenmistir).';
    }
    if (c.indexOf('media_url_unavailable') !== -1) {
        return 'Instagram, icerigi herkese acik bir medya adresinden yayinlamayi gerektirir. Bu sunucusuz masaustu mimaride medya barindirma / herkese acik URL hizmeti bulunmadigi icin Instagram yayini yapilamaz; sahte basari uretilmez.';
    }
    if (c.indexOf('permission_denied') !== -1) {
        return 'Platform izin reddetti. Uygulama gerekli izinlere sahip degil (App Review / izin yapilandirmasi gerekebilir).';
    }
    if (c.indexOf('app_review_required') !== -1) {
        return 'Bu islem icin Meta App Review onayi gerekiyor. Uygulama henuz inceleme onayina sahip degil.';
    }
    if (c.indexOf('instagram_professional_account_required') !== -1) {
        return 'Instagram yayini icin Isletme veya Yazar (profesyonel) hesap baglantisi gerekir. Kisisel profil veya profesyonel hesaba bagli olmayan hedefle yayin yapilamaz.';
    }
    if (c.indexOf('no_managed_page') !== -1) {
        return 'Bu hesaba bagli yonetilen bir Facebook Sayfasi bulunamadi. Yayin icin bir Sayfa gerekir.';
    }
    if (c.indexOf('page_not_found') !== -1) {
        return 'Hedef Facebook Sayfasi bulunamadi.';
    }
    if (c.indexOf('instagram_account_not_found') !== -1) {
        return 'Hedef platformda bagli bir Instagram hesabi bulunamadi.';
    }
    if (c.indexOf('token_expired') !== -1) {
        return 'Baglanti tokeninin suresi dolmus. Yeniden baglanmaniz gerekir.';
    }
    if (c.indexOf('token_missing') !== -1) {
        return 'Baglanti tokeni bulunamadi. Hesap bagli degil veya token silinmis.';
    }
    if (c.indexOf('invalid_media_file') !== -1) {
        return 'Secilen medya dosyasi gecersiz (bicim veya sihirli imza uyusmazligi). Yayin yapilmadi, sahte basari gosterilmedi.';
    }
    if (c.indexOf('unsupported_post_type') !== -1) {
        return 'Bu icerik turu hedef platformda desteklenmiyor (ornegin medyasiz/metin duyurusu sosyal medyaya gonderilmez).';
    }
    if (c.indexOf('invalid_connection') !== -1) {
        return 'Gecersiz baglanti. Hesap bagli degil veya baglantisi kopmus.';
    }
    if (c.indexOf('publish_failed') !== -1 || c.indexOf('api_error') !== -1 || c.indexOf('operation_failed') !== -1) {
        return 'Yayin islemi sirasinda bir hata olustu. Lutfen daha sonra tekrar deneyin.';
    }
    if (c.indexOf('media_container_failed') !== -1) {
        return 'Instagram medya container olusturulamadi.';
    }
    if (c.indexOf('media_processing_timeout') !== -1) {
        return 'Medya isleme suresi asimina ugradi (TikTok yayin durumu yoklanirken zaman asimi).';
    }
    if (c.indexOf('invalid_video_file') !== -1 || c.indexOf('file_not_found') !== -1) {
        return 'TikTok video yayini icin gecerli bir video dosyasi gerekir. Secilen dosya bulunamadi veya gecersiz bir video (sihirli imza) iceriyor.';
    }
    if (c.indexOf('upload_session_failed') !== -1) {
        return 'TikTok video yayin oturumu baslatilamadi. Uygulamanin video.publish izni ve gecerli bir baglanti oldugundan emin olun.';
    }
    if (c.indexOf('upload_failed') !== -1) {
        return 'TikTok video dosyasi presigned adrese yuklenemedi. Baglanti veya dosya boyutu sorunu olabilir.';
    }
    return 'Islem gerceklestirilemedi. Lutfen daha sonra tekrar deneyin.';
}

// Meta (Facebook/Instagram) bağlantı akışı için tekrarlı bildirim koruması.
// `metaBaglantiBeklemede`: devam eden bir OAuth akışı varken yeni tıklamalar
// yok sayılır (çift tıklamada tek bildirim üretilir).
// `sonMetaHata`: aynı yapılandırma hatası kısa süre içinde art arda tekrar
// edilirse bildirim çoğaltılmaz.
var metaBaglantiBeklemede = null;
var sonMetaHata = {};

function ayarlarPlatformBaglan(id) {
    var p = ayarlarPlatformBul(id);
    if (!p) return;

    // Platformun destek durumu yalniz Rust (registry) katalogundan alinir.
    // Bu, tek yetkili teknik kaynaktir; JavaScript tarafinda bagimsiz bir
    // teknik destek haritasi tutulmaz.
    var support = sosyalKatalog[id] ? sosyalKatalog[id].support_status : null;

    if (support === 'unsupported') {
        bildirimEkle('sosyal-medya-baglanti', 'uyari',
            p.ad + ' desteklenmiyor',
            p.ad + ' bu platform mevcut sunucusuz mimaride desteklenmemektedir.');
        return;
    }

    // X: gercek OAuth 1.0a akisi Rust (`x_connect`) tarafindan calistirilir.
    // Kimlikler EXE'ye gomulu oldugundan on-kontrol yapilmaz; eksikse Rust
    // `x_not_configured` kontrollu hatasiyla doner (sahte basari uretilmez).
    if (id === 'x') {
        var xConn = esTauriInvoke('x_connect', {});
        if (!xConn) {
            bildirimEkle('sosyal-medya-baglanti', 'bilgi',
                'X baglantisi yalniz masaustunde kullanilabilir',
                'X hesap baglantisi icin ES OPS masaustu uygulamasi gerekir.');
            return;
        }
        xConn.then(function(res) {
            var durum = res && res.connection ? res.connection.connectionStatus : null;
            if (durum === 'connected') {
                p.bagli = true;
                p.hesapAdi = res.connection.accountDisplayName || '';
                p.sonKontrol = new Date().toLocaleString('tr-TR');
                ayarlarPlatformListele();
                dashboardBaglantiGuncelle();
                bildirimEkle('sosyal-medya-baglanti', 'basarili',
                    'X baglantisi kuruldu',
                    'X hesap baglantisi basariyla kuruldu.');
            } else {
                bildirimEkle('sosyal-medya-baglanti', 'uyari',
                    'X baglantisi kurulamadi',
                    'X hesap baglantisi saglanamadi. Sayfayi yenileyin ve tekrar deneyin.');
            }
        }).catch(function(err) {
            var raw = String((err && (err.message || err.code || err)) || '');
            var msg;
            if (raw.indexOf('x_not_configured') !== -1) {
                msg = 'X icin gerekli uygulama kimlikleri bu surumde yapilandirilmamis. Lutfen destek ekibiyle iletisime gecin.';
            } else if (raw.indexOf('oauth_cancelled') !== -1) {
                msg = 'X giris ekraninda yetkilendirme iptal edildi.';
            } else if (raw.indexOf('oauth_timeout') !== -1) {
                msg = 'X yetkilendirme beklenirken zaman asimi oldu. Tekrar deneyin.';
            } else {
                msg = 'X baglantisi basarisiz oldu: ' + raw;
            }
            bildirimEkle('sosyal-medya-baglanti', 'hata',
                'X baglantisi kurulamadi', msg);
        });
        return;
    }

    // YouTube: gercek OAuth akisi Rust (`youtube_connect`) tarafindan calistirilir.
    // Tauri ortami (ve derlenmis client id) yoksa sahte baglanti uretilmez;
    // kullanici dostu bir bilgiyle donulur.
    if (id === 'youtube') {
        var connPromise = esTauriInvoke('youtube_connect');
        if (!connPromise) {
            bildirimEkle('sosyal-medya-baglanti', 'bilgi',
                'YouTube baglantisi yalniz masaustunde kullanilabilir',
                'YouTube hesap baglantisi icin ES OPS masaustu uygulamasi gerekir.');
            return;
        }
        // OAuth akisi devam ederken tarayici resmi Google giris ekranina acilir.
        connPromise.then(function(res) {
            if (res && res.connection && res.connection.connectionStatus === 'connected') {
                p.bagli = true;
                p.hesapAdi = res.connection.accountDisplayName || '';
                p.sonKontrol = new Date().toLocaleString('tr-TR');
                ayarlarPlatformListele();
                dashboardBaglantiGuncelle();
                bildirimEkle('sosyal-medya-baglanti', 'basarili',
                    'YouTube baglantisi kuruldu',
                    p.ad + ' hesap baglantisi basariyla kuruldu.');
            } else {
                bildirimEkle('sosyal-medya-baglanti', 'hata',
                    'YouTube baglantisi kurulamadi',
                    'YouTube baglanti islemi basarisiz oldu. Lutfen tekrar deneyin.');
            }
        }).catch(function(err) {
            // Rust tarafindan kontrollu hata kodlari dondurulur; kullaniciya
            // teknik kod gosterilmez, anlasilir bir mesaj kurulur.
            var msg = 'YouTube baglanti islemi basarisiz oldu. Lutfen tekrar deneyin.';
            var raw = (err && (err.message || err.code || err)) || '';
            var code = String(raw);
            if (code.indexOf('youtube_not_configured') !== -1) {
                msg = 'YouTube baglanti surumu bu calistirma icin yapilandirilmamis (client id tanimli degil).';
            } else if (code.indexOf('oauth_cancelled') !== -1) {
                msg = 'YouTube giris ekraninda yetkilendirme iptal edildi.';
            } else if (code.indexOf('oauth_timeout') !== -1) {
                msg = 'YouTube giris oturumu zaman asimina ugradi. Yeniden deneyin.';
            }
            bildirimEkle('sosyal-medya-baglanti', 'hata',
                'YouTube baglantisi kurulamadi', msg);
        });
        return;
    }

    // Facebook ve Instagram: gerçek Meta OAuth akışına bağlanır.
    // Yapılandırma durumu Rust `assert_connect_ready` kapısında tek noktada
    // denetlenir; JavaScript ayrı bir ön kontrol yapmaz. Böylece tek tıklamada
    // yalnızca tek bildirim oluşur ve "App ID mevcutken eksik hatası" verilmez.
    // Eksik yapılandırmada tarayıcı açılmaz; kontrollü tek hata kodu
    // (`meta_not_configured` / `app_secret_required`) kullanıcı dostu bir
    // bildirime dönüştürülür. Kullanıcı adı alanı bu akışta kullanılmaz.
    if (id === 'facebook' || id === 'instagram') {
        // Devam eden bir Meta akışı varken yeni tıklama yok sayılır.
        if (metaBaglantiBeklemede) return;
        metaBaglantiBeklemede = { platform: id };

        var metaCmd = (id === 'facebook') ? 'facebook_connect' : 'instagram_connect';
        var metaConn = esTauriInvoke(metaCmd, {});
        if (!metaConn) {
            metaBaglantiBeklemede = null;
            bildirimEkle('sosyal-medya-baglanti', 'bilgi',
                p.ad + ' bağlantısı yalnız masaüstünde kullanılabilir',
                p.ad + ' hesap bağlantısı için ES OPS masaüstü uygulaması gerekir.');
            return;
        }
        metaConn.then(function(res) {
            metaBaglantiBeklemede = null;
            var durum = res && res.connection ? res.connection.connectionStatus : null;
            if (durum === 'connected') {
                p.bagli = true;
                // OAuth sonrası dönen gerçek hesap adı görüntülenir.
                p.hesapAdi = res.connection.accountDisplayName || '';
                p.sonKontrol = new Date().toLocaleString('tr-TR');
                ayarlarPlatformListele();
                dashboardBaglantiGuncelle();
                bildirimEkle('sosyal-medya-baglanti', 'basarili',
                    p.ad + ' bağlantısı kuruldu',
                    p.ad + ' hesap bağlantısı başarıyla kuruldu.');
            } else if (durum === 'token_expired') {
                bildirimEkle('sosyal-medya-baglanti', 'uyari',
                    p.ad + ' yetkilendirme gerekli',
                    metaHataMesaji('reauthorization_required'));
            } else {
                bildirimEkle('sosyal-medya-baglanti', 'uyari',
                    p.ad + ' bağlantısı kurulamadı',
                    p.ad + ' hesap bağlantısı sağlanamadı. Sayfayı yenileyin ve tekrar deneyin.');
            }
        }).catch(function(err) {
            metaBaglantiBeklemede = null;
            var raw = String((err && (err.message || err.code || err)) || '');
            // Yapılandırma hatası geldikçe kullanıcıdan kullanıcı adı/şifre
            // istenmez; tek ve anlaşılır bir bildirim verilir.
            var simdi = Date.now();
            var onceki = sonMetaHata[id];
            if (onceki && onceki.kod === raw && (simdi - onceki.zaman) < 10000) {
                return; // aynı hata kısa sürede tekrar tekrar kaydedilmez
            }
            sonMetaHata[id] = { kod: raw, zaman: simdi };
            bildirimEkle('sosyal-medya-baglanti', 'hata',
                p.ad + ' bağlantısı başlatılamadı',
                metaHataMesaji(raw));
        });
        return;
    }

    // TikTok: gercek TikTok Content Posting API OAuth akisina baglanir.
    // Kimlikler EXE'ye gomuldugunden on-kontrol yapilmaz; eksikse Rust
    // `tiktok_not_configured` kontrollu hatasiyla doner. OAuth baslatilmaz,
    // sahte baglanti uretilmez.
    if (id === 'tiktok') {
        var ttConn = esTauriInvoke('tiktok_connect', {});
        if (!ttConn) {
            bildirimEkle('sosyal-medya-baglanti', 'bilgi',
                'TikTok baglantisi yalniz masaustunde kullanilabilir',
                'TikTok hesap baglantisi icin ES OPS masaustu uygulamasi gerekir.');
            return;
        }
        ttConn.then(function(res) {
            var durum = res && res.connection ? res.connection.connectionStatus : null;
            if (durum === 'connected') {
                p.bagli = true;
                p.hesapAdi = res.connection.accountDisplayName || '';
                p.sonKontrol = new Date().toLocaleString('tr-TR');
                ayarlarPlatformListele();
                dashboardBaglantiGuncelle();
                bildirimEkle('sosyal-medya-baglanti', 'basarili',
                    'TikTok baglantisi kuruldu',
                    'TikTok hesap baglantisi basariyla kuruldu.');
            } else {
                bildirimEkle('sosyal-medya-baglanti', 'uyari',
                    'TikTok baglantisi kurulamadi',
                    'TikTok hesap baglantisi saglanamadi. Sayfayi yenileyin ve tekrar deneyin.');
            }
        }).catch(function(err) {
            var raw = (err && (err.message || err.code || err)) || '';
            var code = String(raw);
            var msg;
            if (code.indexOf('tiktok_not_configured') !== -1) {
                msg = 'TikTok icin gerekli uygulama kimlikleri bu surumde yapilandirilmamis. Lutfen destek ekibiyle iletisime gecin.';
            } else if (code.indexOf('oauth_cancelled') !== -1) {
                msg = 'TikTok giris ekraninda yetkilendirme iptal edildi.';
            } else if (code.indexOf('oauth_timeout') !== -1) {
                msg = 'TikTok giris oturumu zaman asimina ugradi. Yeniden deneyin.';
            } else if (code.indexOf('oauth_state_mismatch') !== -1) {
                msg = 'Guvenlik dogrulamasi (state) uyusmazligi. Tekrar deneyin.';
            } else if (code.indexOf('permission_denied') !== -1) {
                msg = 'TikTok izin istegini reddetti. Gerekli kapsamlar (video.publish) onaylanmamis olabilir.';
            } else {
                msg = 'TikTok baglanti islemi basarisiz oldu. Lutfen tekrar deneyin.';
            }
            bildirimEkle('sosyal-medya-baglanti', 'hata',
                'TikTok baglantisi kurulamadi', msg);
        });
        return;
    }

    // LinkedIn: gercek Native PKCE OAuth akisina baglanir (Client Secret
    // gerekmez). Client ID EXE'ye gomuldugunden on-kontrol yapilmaz; eksikse
    // Rust `linkedin_not_configured` kontrollu hatasiyla doner.
    if (id === 'linkedin') {
        var liConn = esTauriInvoke('linkedin_connect', {});
        if (!liConn) {
            bildirimEkle('sosyal-medya-baglanti', 'bilgi',
                'LinkedIn baglantisi yalniz masaustunde kullanilabilir',
                'LinkedIn hesap baglantisi icin ES OPS masaustu uygulamasi gerekir.');
            return;
        }
        // Client ID hazir: gercek PKCE OAuth akisini baslat. Tarayici resmi
        // LinkedIn yetkilendirme sayfasina acilir; callback sonrasi kisisel
        // profil ve yayin yapilabilir sirket sayfalari baglanir.
        liConn.then(function(res) {
            var list = (res && res.connections) || [];
            if (list.length === 0) {
                bildirimEkle('sosyal-medya-baglanti', 'uyari',
                    'LinkedIn baglantisi kurulamadi',
                    'LinkedIn hesap baglantisi saglanamadi. Sayfayi yenileyin ve tekrar deneyin.');
                return;
            }
            p.bagli = true;
            p.hesapAdi = (list[0].accountDisplayName) || '';
            p.sonKontrol = new Date().toLocaleString('tr-TR');
            ayarlarPlatformListele();
            dashboardBaglantiGuncelle();
            var hedefSayisi = '1 kisisel profil';
            var sayfaSayisi = list.length - 1;
            if (sayfaSayisi > 0) hedefSayisi += ' + ' + sayfaSayisi + ' sirket sayfasi';
            bildirimEkle('sosyal-medya-baglanti', 'basarili',
                'LinkedIn baglantisi kuruldu',
                'LinkedIn hesap baglantisi basariyla kuruldu (' + hedefSayisi + ').');
        }).catch(function(err) {
            var raw = String((err && (err.message || err.code || err)) || '');
            var code = String(raw);
            var msg;
            if (code.indexOf('linkedin_not_configured') !== -1) {
                msg = 'LinkedIn icin gerekli uygulama kimligi bu surumde yapilandirilmamis. Lutfen destek ekibiyle iletisime gecin.';
            } else if (code.indexOf('oauth_cancelled') !== -1) {
                msg = 'LinkedIn giris ekraninda yetkilendirme iptal edildi.';
            } else if (code.indexOf('oauth_timeout') !== -1) {
                msg = 'LinkedIn yetkilendirme beklenirken zaman asimi oldu. Tekrar deneyin.';
            } else if (code.indexOf('oauth_state_mismatch') !== -1) {
                msg = 'Guvenlik dogrulamasi (state) uyusmazligi. Tekrar deneyin.';
            } else if (code.indexOf('permission_denied') !== -1) {
                msg = 'LinkedIn izin istegini reddetti. Gerekli izinler (w_member_social / w_organization_social) onaylanmamis olabilir.';
            } else if (code.indexOf('linkedin_identity_lookup_failed') !== -1) {
                msg = 'LinkedIn kullanici kimligi alinamadi. Yetkilendirme yari kalabilir; tekrar deneyin.';
            } else {
                msg = 'LinkedIn baglantisi basarisiz oldu: ' + raw;
            }
            bildirimEkle('sosyal-medya-baglanti', 'hata',
                'LinkedIn baglantisi kurulamadi', msg);
        });
        return;
    }

    // TikTok: gercek OAuth akisi.
    // planned / restricted / verification_pending / tanimsiz:
    // Baglanti henuz etkin degil. Sahte baglanti veya token olusturulmaz,
    // OAuth baslatilmaz, baglanti "kuruldu" gibi gosterilmez.
    bildirimEkle('sosyal-medya-baglanti', 'bilgi',
        p.ad + ' bağlantısı henüz etkin değil',
        'Bu platformun bağlantısı henüz etkinleştirilmedi. Mevcut sürümde hesap bağlama işlemi yapılamamaktadır.');
}

// Mevcut gercek baglanti durumlarini Rust `social_account_connections`
// komutundan yukleyip ilgili platformlari "Bagli" olarak isaretler.
// Baglantisi bulunmayan platformlar "Bagli Degil" olarak kalmaya devam eder.
function sosyalBaglantiDurumlariYukle() {
    var p = esTauriInvoke('social_account_connections');
    if (!p) return; // Tauri ortami yok: liste bos kalir, hata uretilmez
    p.then(function(list) {
        if (!list) return;
        list.forEach(function(c) {
            var plat = ayarlarPlatformBul(c.platformId);
            if (plat && c.connectionStatus === 'connected') {
                plat.bagli = true;
                plat.hesapAdi = c.accountDisplayName || plat.hesapAdi;
                plat.sonKontrol = new Date().toLocaleString('tr-TR');
            }
        });
        ayarlarPlatformListele();
        dashboardBaglantiGuncelle();
    }).catch(function() {});
}

function ayarlarPlatformKes(id) {
    var p = ayarlarPlatformBul(id);
    if (!p) return;

    // Bağlı olmayan hesapta bağlantı kesme başlatılmaz; işlem yapılmadan
    // anlaşılır bir bilgi verilir (sahte "kesildi" üretilmez).
    if (!p.bagli) {
        bildirimEkle('sosyal-medya-baglanti', 'bilgi',
            p.ad + ' hesabı bağlı değil',
            p.ad + ' platformuna bağlı bir hesap bulunmadığı için bağlantı kesme işlemi yapılmadı.');
        return;
    }

    // Baglanti listesi Tauri komutundan alinir. Tauri ortami yoksa
    // (onizleme) baglanti ozelliklerinin yalniz masaustunde kullanildigi
    // kullanici dostu sekilde bildirilir; yakalanmamis hata olusmaz.
    var listPromise = esTauriInvoke('social_account_connections');
    if (!listPromise) {
        bildirimEkle('sosyal-medya-baglanti', 'bilgi',
            'Baglanti yalniz masaustunde kullanilabilir',
            'Sosyal medya hesap baglantilarini yonetmek icin ES OPS masaustu uygulamasi gerekir.');
        return;
    }

    listPromise.then(function(list) {
        var matches = (list || []).filter(function(c) { return c.platformId === id; });

        // Gercek bir connection_id yok: saglan baglanti kesme basarisi gosterilmez.
        if (matches.length === 0) {
            bildirimEkle('sosyal-medya-baglanti', 'uyari',
                'Bagli hesap bulunmuyor',
                p.ad + ' platformuna bagli bir hesap bulunmadigi icin baglanti kesme islemi yapilamadi.');
            return;
        }

        var connId = matches[0].connectionId;
        var disPromise = esTauriInvoke('social_disconnect_account', { connectionId: connId });
        if (!disPromise) return;
        disPromise.then(function(res) {
            if (res && res.status === 'disconnected') {
                bildirimEkle('sosyal-medya-baglanti', 'basarili',
                    p.ad + ' baglantisi kesildi',
                    p.ad + ' hesap baglantisi basariyla kesildi.');
            } else if (res && res.status === 'not_connected') {
                bildirimEkle('sosyal-medya-baglanti', 'uyari',
                    p.ad + ' hesap bagli degil',
                    'Hesap zaten bagli olmadigi icin baglanti kesme islemi yapilamadi.');
            } else if (res && res.status === 'not_found') {
                bildirimEkle('sosyal-medya-baglanti', 'uyari',
                    p.ad + ' baglantisi bulunamadi',
                    'Baglanti kaydi bulunamadi. Islem yapilamadi.');
            } else {
                bildirimEkle('sosyal-medya-baglanti', 'hata',
                    p.ad + ' baglantisi kesilemedi',
                    'Baglanti kesme islemi sirasinda bir hata olustu. Lutfen tekrar deneyin.');
            }
        }).catch(function() {
            bildirimEkle('sosyal-medya-baglanti', 'hata',
                p.ad + ' baglantisi kesilemedi',
                'Baglanti kesme islemi sirasinda bir hata olustu.');
        });
    }).catch(function() {
        bildirimEkle('sosyal-medya-baglanti', 'hata',
            'Baglanti islemi gerceklestirilemedi',
            'Baglantili hesap listesi okunamadi. Lutfen tekrar deneyin.');
    });
}

// ===== WEB SITESI BAGLANTISI =====

// Web yapilandirmasini form alanlarindan toplar. Gizli olmayan tum alanlar
// SiteConfigInput ile birebir eslesir; secret yalniz doluysa gonderilir.
function webAyarlarFormuGetir() {
    var timeout = parseInt(document.getElementById('ayarlarWebSure').value, 10);
    if (isNaN(timeout) || timeout <= 0) timeout = 15;
    return {
        site_url: document.getElementById('ayarlarWebAdres').value.trim(),
        api_base: document.getElementById('ayarlarApiAdres').value.trim(),
        auth_style: document.getElementById('ayarlarAuthYontem').value,
        auth_header_name: document.getElementById('ayarlarAuthBaslik').value.trim(),
        test_endpoint: document.getElementById('ayarlarWebTestEndpoint').value.trim(),
        publish_endpoint: document.getElementById('ayarlarWebYayinEndpoint').value.trim(),
        sections_endpoint: document.getElementById('ayarlarWebBolumEndpoint').value.trim(),
        sections_path: document.getElementById('ayarlarWebSectionsPath').value.trim() || 'data',
        section_id_path: document.getElementById('ayarlarWebBolumIdPath').value.trim() || 'id',
        section_name_path: document.getElementById('ayarlarWebBolumAdiPath').value.trim() || 'name',
        content_id_path: document.getElementById('ayarlarWebIcerikIdPath').value.trim() || 'data.id',
        content_url_path: document.getElementById('ayarlarWebIcerikUrlPath').value.trim() || 'data.url',
        error_message_path: document.getElementById('ayarlarWebHataMesajPath').value.trim() || 'error.message',
        media_field_name: document.getElementById('ayarlarWebMedyaAlanAdi').value.trim() || 'image',
        multipart: document.getElementById('ayarlarWebMultipart').checked,
        timeout_seconds: timeout,
        payload_template: document.getElementById('ayarlarWebSablon').value.trim(),
        secret: document.getElementById('ayarlarApiAnahtar').value
    };
}

function webAyarlarDoldur(view) {
    if (!view) return;
    var cfg = view.cfg || view;
    if (document.getElementById('ayarlarWebAdres')) document.getElementById('ayarlarWebAdres').value = cfg.site_url || '';
    if (document.getElementById('ayarlarApiAdres')) document.getElementById('ayarlarApiAdres').value = cfg.api_base || '';
    if (document.getElementById('ayarlarAuthYontem')) document.getElementById('ayarlarAuthYontem').value = cfg.auth_style || '';
    if (document.getElementById('ayarlarAuthBaslik')) document.getElementById('ayarlarAuthBaslik').value = cfg.auth_header_name || '';
    if (document.getElementById('ayarlarWebTestEndpoint')) document.getElementById('ayarlarWebTestEndpoint').value = cfg.test_endpoint || '';
    if (document.getElementById('ayarlarWebYayinEndpoint')) document.getElementById('ayarlarWebYayinEndpoint').value = cfg.publish_endpoint || '';
    if (document.getElementById('ayarlarWebBolumEndpoint')) document.getElementById('ayarlarWebBolumEndpoint').value = cfg.sections_endpoint || '';
    if (document.getElementById('ayarlarWebSectionsPath')) document.getElementById('ayarlarWebSectionsPath').value = cfg.sections_path || '';
    if (document.getElementById('ayarlarWebBolumIdPath')) document.getElementById('ayarlarWebBolumIdPath').value = cfg.section_id_path || '';
    if (document.getElementById('ayarlarWebBolumAdiPath')) document.getElementById('ayarlarWebBolumAdiPath').value = cfg.section_name_path || '';
    if (document.getElementById('ayarlarWebIcerikIdPath')) document.getElementById('ayarlarWebIcerikIdPath').value = cfg.content_id_path || '';
    if (document.getElementById('ayarlarWebIcerikUrlPath')) document.getElementById('ayarlarWebIcerikUrlPath').value = cfg.content_url_path || '';
    if (document.getElementById('ayarlarWebHataMesajPath')) document.getElementById('ayarlarWebHataMesajPath').value = cfg.error_message_path || '';
    if (document.getElementById('ayarlarWebMedyaAlanAdi')) document.getElementById('ayarlarWebMedyaAlanAdi').value = cfg.media_field_name || '';
    if (document.getElementById('ayarlarWebMultipart')) document.getElementById('ayarlarWebMultipart').checked = !!cfg.multipart;
    if (document.getElementById('ayarlarWebSure')) document.getElementById('ayarlarWebSure').value = cfg.timeout_seconds || 15;
    if (document.getElementById('ayarlarWebSablon')) document.getElementById('ayarlarWebSablon').value = cfg.payload_template || '';
    ayarlarWebBaglanti.bagli = false;
    ayarlarWebBaglanti.webAdres = cfg.site_url || '';
    ayarlarWebBaglanti.apiAdres = cfg.api_base || '';
    ayarlarWebBaglanti.authYontem = cfg.auth_style || '';
    ayarlarWebBaglanti.sonKontrol = view.last_test || '';
    ayarlarWebDurumGoster(cfg.api_base !== '');
}

function ayarlarWebTestEt() {
    var input = webAyarlarFormuGetir();
    if (!input.api_base || !input.test_endpoint || !input.publish_endpoint || !input.auth_style) {
        var mesaj = 'API temel adresi, test uc noktasi, yayin uc noktasi ve dogrulama yontemi zorunludur.';
        alert(mesaj);
        bildirimEkle('sistem-uyari', 'uyari', 'Web baglantisi testi basarisiz - Zorunlu alan eksik', mesaj);
        return;
    }
    var p = esTauriInvoke('website_config_save', { input: input });
    if (!p) {
        alert('Baglanti testi yalniz uygulama ici calistiginda yapilabilir.');
        return;
    }
    p.then(function() {
        return esTauriInvoke('website_test');
    }).then(function(out) {
        if (out && out.ok) {
            ayarlarWebBaglanti.bagli = true;
            ayarlarWebBaglanti.sonKontrol = new Date().toLocaleString('tr-TR');
            ayarlarWebDurumGoster(true);
            dashboardBaglantiGuncelle();
            alert('Baglanti basarili.\n' + out.turkce);
            bildirimEkle('web-baglanti', 'basarili', 'Web baglantisi testi basarili', out.turkce);
        } else {
            var msg = out ? out.turkce : 'Test sonucu alinamadi.';
            alert('Baglanti testi basarisiz.\n' + msg);
            bildirimEkle('web-baglanti', 'uyari', 'Web baglantisi testi basarisiz', msg);
        }
    }).catch(function(e) {
        var msg = (e && e.message) ? e.message : (typeof e === 'string' ? e : 'Bilinmeyen hata');
        alert('Baglanti testi basarisiz.\n' + msg);
        bildirimEkle('web-baglanti', 'uyari', 'Web baglantisi testi basarisiz', msg);
    });
}

function ayarlarWebKaydet() {
    var input = webAyarlarFormuGetir();
    if (!input.api_base || !input.test_endpoint || !input.publish_endpoint || !input.auth_style) {
        var mesaj = 'API temel adresi, test uc noktasi, yayin uc noktasi ve dogrulama yontemi zorunludur.';
        alert(mesaj);
        bildirimEkle('sistem-uyari', 'uyari', 'Web baglantisi kaydedilemedi - Zorunlu alan eksik', mesaj);
        return;
    }

    // Onizleme modunda gizli olmayan bilgileri localStorage'a yedekle (anahtar asla).
    var kayit = {
        bagli: false,
        webAdres: input.site_url,
        apiAdres: input.api_base,
        authYontem: input.auth_style,
        sonKontrol: new Date().toLocaleString('tr-TR')
    };
    localStorage.setItem(AYARLAR_WEB_KEY, JSON.stringify(kayit));

    var p = esTauriInvoke('website_config_save', { input: input });
    if (!p) {
        ayarlarWebBaglanti.bagli = false;
        ayarlarWebBaglanti.webAdres = input.site_url;
        ayarlarWebBaglanti.apiAdres = input.api_base;
        ayarlarWebBaglanti.authYontem = input.auth_style;
        ayarlarWebDurumGoster(input.api_base !== '');
        document.getElementById('ayarlarApiAnahtar').value = '';
        alert('Web yapilandirmasi kaydedildi. API anahtari saklanmamistir (onizleme modu).');
        return;
    }
    p.then(function() {
        ayarlarWebBaglanti.bagli = false;
        ayarlarWebBaglanti.webAdres = input.site_url;
        ayarlarWebBaglanti.apiAdres = input.api_base;
        ayarlarWebBaglanti.authYontem = input.auth_style;
        ayarlarWebBaglanti.sonKontrol = new Date().toLocaleString('tr-TR');
        ayarlarWebDurumGoster(true);
        dashboardBaglantiGuncelle();
        // API anahtari localStorage ya da console'a yazilmaz
        document.getElementById('ayarlarApiAnahtar').value = '';
        alert('Web yapilandirmasi kaydedildi. Anahtar Windows Kimlik Bilgi Yoneticisi icinde saklanir.');
        bildirimEkle('web-baglanti', 'basarili', 'Web yapilandirmasi kaydedildi',
            'API temel adresi ve uc noktalari kaydedildi; anahtar guvenli depoda saklanir.');
    }).catch(function(e) {
        var msg = (e && e.message) ? e.message : (typeof e === 'string' ? e : 'Kayit sirasinda hata olustu');
        alert('Kaydetme basarisiz.\n' + msg);
        bildirimEkle('sistem-uyari', 'uyari', 'Web yapilandirmasi kaydedilemedi', msg);
    });
}

function ayarlarWebKaldir() {
    var temizle = function() {
        ayarlarWebBaglanti = {
            bagli: false,
            webAdres: '',
            apiAdres: '',
            authYontem: 'bearer',
            sonKontrol: ''
        };
        if (document.getElementById('ayarlarWebAdres')) document.getElementById('ayarlarWebAdres').value = '';
        if (document.getElementById('ayarlarApiAdres')) document.getElementById('ayarlarApiAdres').value = '';
        if (document.getElementById('ayarlarAuthYontem')) document.getElementById('ayarlarAuthYontem').value = 'bearer';
        if (document.getElementById('ayarlarAuthBaslik')) document.getElementById('ayarlarAuthBaslik').value = '';
        if (document.getElementById('ayarlarWebTestEndpoint')) document.getElementById('ayarlarWebTestEndpoint').value = '';
        if (document.getElementById('ayarlarWebYayinEndpoint')) document.getElementById('ayarlarWebYayinEndpoint').value = '';
        if (document.getElementById('ayarlarWebBolumEndpoint')) document.getElementById('ayarlarWebBolumEndpoint').value = '';
        if (document.getElementById('ayarlarApiAnahtar')) document.getElementById('ayarlarApiAnahtar').value = '';
        localStorage.removeItem(AYARLAR_WEB_KEY);
        ayarlarWebDurumGoster(false);
        dashboardBaglantiGuncelle();
        alert('Web baglantisi ve guvenli anahtar kaldirildi.');
    };
    var p = esTauriInvoke('website_config_clear');
    if (!p) { temizle(); return; }
    p.then(temizle).catch(function(e) {
        var msg = (e && e.message) ? e.message : 'Bilinmeyen hata';
        alert('Kaldirilemedi.\n' + msg);
    });
}

function ayarlarWebDurumGoster(configured) {
    var durumGrubu = document.getElementById('ayarlarWebDurumGrubu');
    var durumEl = document.getElementById('ayarlarWebDurum');
    var sonKontrolEl = document.getElementById('ayarlarWebSonKontrol');
    var kaldirBtn = document.getElementById('ayarlarWebKaldirBtn');

    if (!durumGrubu || !durumEl || !sonKontrolEl || !kaldirBtn) return;

    if (configured) {
        durumGrubu.style.display = 'block';
        if (ayarlarWebBaglanti.bagli) {
            durumEl.innerHTML = '<span class="status-dot green"></span> Baglanildi (test basarili)';
        } else {
            durumEl.innerHTML = '<span class="status-dot gray"></span> Yapilandirildi - Test edilmedi';
        }
        sonKontrolEl.textContent = 'Son kontrol: ' + (ayarlarWebBaglanti.sonKontrol || '-');
        kaldirBtn.style.display = 'inline-block';
    } else {
        durumGrubu.style.display = 'none';
        kaldirBtn.style.display = 'none';
    }
}

function ayarlarWebGeriYukle() {
    var p = esTauriInvoke('website_config_get');
    if (!p) {
        // Onizleme modu: yalniz gizli olmayan eski yerel kayit doldurulur
        var kayitli = localStorage.getItem(AYARLAR_WEB_KEY);
        if (kayitli) {
            try {
                var data = JSON.parse(kayitli);
                ayarlarWebBaglanti.bagli = false;
                ayarlarWebBaglanti.webAdres = data.webAdres || '';
                ayarlarWebBaglanti.apiAdres = data.apiAdres || '';
                ayarlarWebBaglanti.authYontem = data.authYontem || 'bearer';
                ayarlarWebBaglanti.sonKontrol = data.sonKontrol || '';
                if (document.getElementById('ayarlarWebAdres')) document.getElementById('ayarlarWebAdres').value = ayarlarWebBaglanti.webAdres;
                if (document.getElementById('ayarlarApiAdres')) document.getElementById('ayarlarApiAdres').value = ayarlarWebBaglanti.apiAdres;
                if (document.getElementById('ayarlarWebTestEndpoint')) document.getElementById('ayarlarWebTestEndpoint').value = '/health';
                ayarlarWebDurumGoster(ayarlarWebBaglanti.apiAdres !== '');
            } catch (e) {}
        }
        return;
    }
    p.then(function(view) {
        if (view) webAyarlarDoldur(view);
    }).catch(function() {});
}

// ===== GENEL AYARLAR =====
function ayarlarGenelKaydet() {
    var ayarlar = {
        firmaAdi: document.getElementById('ayarlarFirmaAdi').value.trim(),
        ulke: document.getElementById('ayarlarUlke').value.trim(),
        saatDilimi: document.getElementById('ayarlarSaatDilimi').value,
        dil: document.getElementById('ayarlarDil').value,
        tarihFormat: document.getElementById('ayarlarTarihFormat').value,
        saatFormat: document.getElementById('ayarlarSaatFormat').value,
        otomatikBaslangic: document.getElementById('ayarlarOtomatikBaslangic').checked,
        bildirimler: document.getElementById('ayarlarBildirimler').checked,
        sistemUyarilari: document.getElementById('ayarlarSistemUyarilari').checked
    };

    localStorage.setItem(AYARLAR_STORAGE_KEY, JSON.stringify(ayarlar));

    alert('Genel ayarlar kaydedildi.');

    bildirimEkle('genel', 'basarili',
        'Genel ayarlar kaydedildi',
        'Genel ayarlar basariyla kaydedildi. Firma: ' + (ayarlar.firmaAdi || 'Belirtilmemis')
    );
}

function ayarlarGenelGeriYukle() {
    var kayitli = localStorage.getItem(AYARLAR_STORAGE_KEY);
    if (kayitli) {
        try {
            var data = JSON.parse(kayitli);
            if (document.getElementById('ayarlarFirmaAdi')) document.getElementById('ayarlarFirmaAdi').value = data.firmaAdi || '';
            if (document.getElementById('ayarlarUlke')) document.getElementById('ayarlarUlke').value = data.ulke || '';
            if (document.getElementById('ayarlarSaatDilimi')) document.getElementById('ayarlarSaatDilimi').value = data.saatDilimi || 'Europe/Istanbul';
            if (document.getElementById('ayarlarDil')) document.getElementById('ayarlarDil').value = data.dil || 'tr';
            if (document.getElementById('ayarlarTarihFormat')) document.getElementById('ayarlarTarihFormat').value = data.tarihFormat || 'DD.MM.YYYY';
            if (document.getElementById('ayarlarSaatFormat')) document.getElementById('ayarlarSaatFormat').value = data.saatFormat || '24';
            if (document.getElementById('ayarlarOtomatikBaslangic')) document.getElementById('ayarlarOtomatikBaslangic').checked = data.otomatikBaslangic !== false;
            if (document.getElementById('ayarlarBildirimler')) document.getElementById('ayarlarBildirimler').checked = data.bildirimler !== false;
            if (document.getElementById('ayarlarSistemUyarilari')) document.getElementById('ayarlarSistemUyarilari').checked = data.sistemUyarilari !== false;
        } catch(e) {}
    } else {
        // Varsayilan degerler
        if (document.getElementById('ayarlarDil')) document.getElementById('ayarlarDil').value = 'tr';
        if (document.getElementById('ayarlarTarihFormat')) document.getElementById('ayarlarTarihFormat').value = 'DD.MM.YYYY';
        if (document.getElementById('ayarlarSaatFormat')) document.getElementById('ayarlarSaatFormat').value = '24';
        if (document.getElementById('ayarlarOtomatikBaslangic')) document.getElementById('ayarlarOtomatikBaslangic').checked = true;
        if (document.getElementById('ayarlarBildirimler')) document.getElementById('ayarlarBildirimler').checked = true;
        if (document.getElementById('ayarlarSistemUyarilari')) document.getElementById('ayarlarSistemUyarilari').checked = true;
    }
}

// Sayfa yuklendiginde ayarlari geri yukle ve dashboard'u guncelle
document.addEventListener('DOMContentLoaded', function() {
    ayarlarPlatformListele();
    ayarlarWebGeriYukle();
    ayarlarGenelGeriYukle();
    sosyalKatalogYukle();
    sosyalBaglantiDurumlariYukle(); // YouTube dahil gercek baglanti durumunu yukle
    dashboardBaglantiGuncelle();
});

// Dashboard baglanti durumunu Ayarlar verisiyle senkronize et
function dashboardBaglantiGuncelle() {
    // Sosyal medya hesaplari
    ayarlarPlatformlar.forEach(function(p) {
        // Statik HTML'deki sirali yapiyi kullan
        var items = document.querySelectorAll('#dash-sosyal-medya .dashboard-card .status-item');
        var platformSirasi = { 'instagram': 0, 'facebook': 1, 'linkedin': 2, 'x': 3, 'tiktok': 4, 'youtube': 5 };
        var idx = platformSirasi[p.id];
        if (idx !== undefined && items[idx]) {
            var valueEl = items[idx].querySelector('.value');
            if (valueEl) {
                valueEl.textContent = p.bagli ? 'Bagli' : 'Bagli Degil';
            }
            var dotEl = items[idx].querySelector('.status-dot');
            if (dotEl) {
                dotEl.className = 'status-dot ' + (p.bagli ? 'green' : 'gray');
            }
        }
    });

    // Web sitesi baglantisi
    var webItems = document.querySelectorAll('#dash-web-sitesi .dashboard-card .status-item');
    if (webItems.length > 0) {
        var webVal = webItems[0].querySelector('.value');
        var webDot = webItems[0].querySelector('.status-dot');
        if (webVal) {
            webVal.textContent = ayarlarWebBaglanti.bagli ? 'Bagli' : 'Bagli Degil';
        }
        if (webDot) {
            webDot.className = 'status-dot ' + (ayarlarWebBaglanti.bagli ? 'green' : 'gray');
        }
    }
}

// ================================================================
// FAZ 9 - LISANS VE 15 GUNLUK DEMO SISTEMI
// ================================================================

// ---- YAPILANDIRMA ----
var ES_LISANS = {
    PRODUCT_CODE: '002',
    PRODUCT_NAME: 'ES Otomatik Paylasim Sistemi',
    DEMO_GUN: 15,
    LICENCE_FILE_NAME: 'license.lic'
};

// ---- KALICI DEPOLAMA (localStorage tabanli) ----
// NOT: localStorage kullanici tarafindan silinebilir.
// Gercek koruma icin isletim sistemi seviyesinde veya sunucu tarafi cozum gerekir.
var ES_DEPO = {
    _prefix: 'es_ops_',

    set: function(key, value) {
        try {
            localStorage.setItem(this._prefix + key, JSON.stringify(value));
        } catch(e) {
            console.error('Depolama hatasi:', e);
        }
    },

    get: function(key, defaultValue) {
        try {
            var val = localStorage.getItem(this._prefix + key);
            return val ? JSON.parse(val) : defaultValue;
        } catch(e) {
            return defaultValue;
        }
    },

    remove: function(key) {
        try {
            localStorage.removeItem(this._prefix + key);
        } catch(e) {}
    },

    // Guvenli depolama alani - trial_start_date ve last_run_date icin
    // NOT: localStorage oldugu icin tam koruma saglamaz.
    // Gercek koruma: Windows Registry veya sunucu tarafi.
    // XOR sifrelemesi sadece merakli gozlerden gizler, guvenli sifreleme degildir.
    // localStorage silindiginde demo sayaci da sifirlanir.
    setSecure: function(key, value) {
        // Basit XOR sifreleme ile gizleme
        var encoded = btoa(unescape(encodeURIComponent(JSON.stringify(value))));
        var salt = 'ESOPS_SALT_2024';
        var mixed = salt.split('').map(function(c, i) {
            return String.fromCharCode(c.charCodeAt(0) ^ (encoded.charCodeAt(i % encoded.length) || 0));
        }).join('');
        var finalVal = btoa(unescape(encodeURIComponent(mixed + '||' + encoded)));
        try {
            localStorage.setItem(this._prefix + 'secure_' + key, finalVal);
        } catch(e) {}
    },

    getSecure: function(key, defaultValue) {
        try {
            var stored = localStorage.getItem(this._prefix + 'secure_' + key);
            if (!stored) return defaultValue;
            var decoded = decodeURIComponent(escape(atob(stored)));
            var parts = decoded.split('||');
            if (parts.length !== 2) return defaultValue;
            var realValue = JSON.parse(decodeURIComponent(escape(atob(parts[1]))));
            return realValue;
        } catch(e) {
            return defaultValue;
        }
    }
};

// ---- MAKINE KIMLIGI (Machine ID) ----
// NOT: Gercek makine kodu Rust/Tauri tarafinda uretilir (ES Merkez Lisanslama
// ile ayni algoritma): wmic CPU ProcessorId + disk SerialNumber +
// hostname-UuidCreateSequential parcalari SHA-256 ile ozetlenir.
// JavaScript yalniz sonucu goruntuler; dogrulama tarafi kesinlikle JS degildir.
// Tarayici onizlemesi (Tauri disi) icin gecici bir fallback parmak izi kullanilir.
var ES_MACHINE = {
    _machineId: null,

    _getFallbackId: function() {
        // Tarayici tabanli benzersiz kimlik olustur
        var navProps = [
            navigator.userAgent || '',
            navigator.language || '',
            navigator.platform || '',
            screen.width || '',
            screen.height || '',
            screen.colorDepth || ''
        ].join('|');

        // Basit hash
        var hash = 0;
        for (var i = 0; i < navProps.length; i++) {
            var char = navProps.charCodeAt(i);
            hash = ((hash << 5) - hash) + char;
            hash = hash & hash; // Convert to 32bit integer
        }
        // Referans license.lic format: MID-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX
        var base = Math.abs(hash).toString(16).toUpperCase().padStart(8, '0');
        var nowHex = Date.now().toString(16).toUpperCase().slice(-8).padStart(8, '0');
        var raw = (base + nowHex + 'ABCD').substring(0, 20).toUpperCase();
        var groups = [];
        for (var g = 0; g < 5; g++) {
            groups.push(raw.substring(g * 4, g * 4 + 4));
        }
        return 'MID-' + groups.join('-');
    },

    // Gercek makine kodu Rust komutu 'license_machine_id' ile alinir.
    // Tauri yoksa (tarayici onizlemesi) gecici fallback kullanilir.
    getMachineId: function() {
        var self = this;
        if (this._machineId) return Promise.resolve(this._machineId);

        var stored = ES_DEPO.get('machine_id', null);
        if (stored) {
            this._machineId = stored;
            return Promise.resolve(stored);
        }

        var p = esTauriInvoke('license_machine_id');
        if (!p) {
            var id = this._getFallbackId();
            this._machineId = id;
            ES_DEPO.set('machine_id', id);
            return Promise.resolve(id);
        }

        return p.then(function(id) {
            self._machineId = id;
            ES_DEPO.set('machine_id', id);
            return id;
        }).catch(function() {
            var fallback = self._getFallbackId();
            self._machineId = fallback;
            ES_DEPO.set('machine_id', fallback);
            return fallback;
        });
    },

    getDisplayId: function() {
        if (this._machineId) return this._machineId;
        var stored = ES_DEPO.get('machine_id', null);
        if (stored) return stored;
        return this._getFallbackId();
    }
};

// ---- DEMO YONETIMI ----
var ES_DEMO = {
    _trialStartKey: 'trial_start',
    _lastRunKey: 'last_run',

    getTrialStart: function() {
        return ES_DEPO.getSecure(this._trialStartKey, null);
    },

    setTrialStart: function(date) {
        ES_DEPO.setSecure(this._trialStartKey, date.toISOString());
    },

    getLastRun: function() {
        return ES_DEPO.getSecure(this._lastRunKey, null);
    },

    setLastRun: function(date) {
        ES_DEPO.setSecure(this._lastRunKey, date.toISOString());
    },

    // Demo baslat / kontrol et
    checkAndStart: function() {
        var trialStart = this.getTrialStart();

        if (!trialStart) {
            // Ilk calistirma - demo basla
            var now = new Date();
            this.setTrialStart(now);
            this.setLastRun(now);
            return { status: 'active', daysLeft: ES_LISANS.DEMO_GUN, startDate: now };
        }

        var startDate = new Date(trialStart);
        var now = new Date();
        var lastRun = this.getLastRun();

        // Tarih geri alma kontrolu
        if (lastRun) {
            var lastRunDate = new Date(lastRun);
            // Sistem tarihi geri alinmis mi?
            if (now < lastRunDate) {
                // Tarih geri alinmis - demo devam etmesin
                // Ama 5 dakikadan az fark varsa saat dilimi farki olabilir
                var diffMs = lastRunDate.getTime() - now.getTime();
                if (diffMs > 300000) { // 5 dakikadan fazla fark
                    return {
                        status: 'expired',
                        daysLeft: 0,
                        startDate: startDate,
                        reason: 'time_reversal',
                        message: 'Sistem tarihi geri alinamaz. Tarih degisikligi tespit edildi.'
                    };
                }
            }
        }

        this.setLastRun(now);

        // Gecen gun sayisi
        var elapsedMs = now.getTime() - startDate.getTime();
        var elapsedDays = Math.floor(elapsedMs / (1000 * 60 * 60 * 24));
        var daysLeft = ES_LISANS.DEMO_GUN - elapsedDays;

        if (daysLeft <= 0) {
            return { status: 'expired', daysLeft: 0, startDate: startDate };
        }

        return { status: 'active', daysLeft: daysLeft, startDate: startDate };
    },

    getEndDate: function() {
        var start = this.getTrialStart();
        if (!start) return null;
        var startDate = new Date(start);
        var endDate = new Date(startDate);
        endDate.setDate(endDate.getDate() + ES_LISANS.DEMO_GUN);
        return endDate;
    }
};

// ---- LISANS DOGRULAMA ----
// Dogrulama tamamen Rust/Tauri tarafinda yapilir (license.rs):
//   - RSA-2048 / RSA-PSS / SHA-256 imza dogrulamasi (ES Merkez public key'i ile)
//   - canonical JSON (alfabetik anahtar sirasi, compact, UTF-8)
//   - product_code === '002' kontrolu
//   - makine kodu eslesmesi
//   - status === 'ACTIVE' kontrolu
//   - sureli lisanslarda bitis tarihi kontrolu
// Lisans icerigi Windows Credential Manager'da (keyring) saklanir.
// JavaScript yalniz sonucu goruntuler; imza/anahtar JS'te bulunmaz.
var ES_LICENSE = {
    _currentInfo: null,

    // lisans.lic icerigini Rust'a gonder, dogrula ve (gecerliyse) sakla.
    // Eski sonuc yapisini (success/message/license/step) korur.
    loadAndVerify: function(fileContent) {
        var p = esTauriInvoke('license_install', { content: fileContent || '' });
        if (!p) {
            return Promise.resolve({
                success: false,
                message: 'Lisans dogrulamasi yalniz Tauri ortaminda calisir.',
                license: null,
                step: 0
            });
        }
        return p.then(function(status) {
            if (status.valid) {
                ES_LICENSE._currentInfo = status.license || null;
                return {
                    success: true,
                    message: 'Lisans basariyla etkinlestirildi.',
                    license: status.license || null,
                    step: 7
                };
            }
            var reason = status.reason || 'invalid';
            return {
                success: false,
                message: ES_LICENSE._mesaj(reason),
                license: null,
                step: ES_LICENSE._adim(reason)
            };
        }).catch(function() {
            return {
                success: false,
                message: 'Lisans dogrulanamadi.',
                license: null,
                step: 3
            };
        });
    },

    // Rust neden kodlarini kullanici dostu mesaja cevirir.
    _mesaj: function(reason) {
        var mesajlar = {
            invalid_json: 'Gecersiz lisans dosyasi.',
            missing_fields: 'Gecersiz lisans dosyasi.',
            invalid_base64: 'Lisans dogrulanamadi.',
            invalid_signature: 'Lisans dogrulanamadi.',
            wrong_product: 'Bu lisans farkli bir urun icin uretilmistir.',
            wrong_machine: 'Bu lisans bu bilgisayar icin gecerli degildir.',
            not_active: 'Lisans durumu gecerli degildir.',
            expired: 'Lisans suresi dolmustur.'
        };
        return mesajlar[reason] || 'Lisans dogrulanamadi.';
    },

    // Hata rengi icin eski adim numaralari: 6+ kirmizi, digerleri sari.
    _adim: function(reason) {
        var adimlar = {
            invalid_json: 2,
            missing_fields: 2,
            invalid_base64: 3,
            invalid_signature: 3,
            wrong_product: 4,
            wrong_machine: 5,
            not_active: 6,
            expired: 7
        };
        return adimlar[reason] || 3;
    },

    // Kayitli lisansi dogrula (program baslangicinda).
    // Rust her cagrida depodaki lisansi yeniden dogrular; gecersizse siler.
    verifyStoredLicense: function() {
        var p = esTauriInvoke('license_status');
        if (!p) {
            return Promise.resolve({ valid: false, reason: 'no_tauri', license: null });
        }
        return p.then(function(status) {
            if (status.valid) {
                ES_LICENSE._currentInfo = status.license || null;
                return { valid: true, reason: null, license: status.license || null };
            }
            ES_LICENSE._currentInfo = null;
            return { valid: false, reason: status.reason || 'invalid', license: null };
        }).catch(function() {
            ES_LICENSE._currentInfo = null;
            return { valid: false, reason: 'error', license: null };
        });
    },

    // Son dogrulanmis lisans bilgisini getir (yalniz gosterim icin).
    getStoredLicense: function() {
        return this._currentInfo;
    },

    // Kayitli lisansi temizle.
    clearStoredLicense: function() {
        this._currentInfo = null;
        esTauriInvoke('license_clear');
    }
};

// ---- ANA BASLANGIC KONTROLU ----
// Program her acilista calistirilir
function esLisansBaslangicKontrolu() {
    var durumKart = document.getElementById('lisansDurumKart');
    var bilgiKart = document.getElementById('lisansBilgiKart');
    var demoBilgi = document.getElementById('lisansDemoBilgi');
    var kilitEkrani = document.getElementById('lisansKilitEkrani');
    var yuklemeAlani = document.getElementById('lisansYuklemeAlani');

    if (!durumKart) return; // Lisans sayfasi yuklenmemis

    // 1. Kayitli lisans var mi ve gecerli mi? (Rust tarafinda dogrulanir)
    ES_LICENSE.verifyStoredLicense().then(function(lisansDurum) {
        if (lisansDurum.valid) {
            // Gecerli lisans var - lisansli mod
            _lisansGoster('licensed', lisansDurum.license);
            return;
        }

        // 2. Lisans yoksa veya gecersizse - demo kontrol et
        var demoDurum = ES_DEMO.checkAndStart();

        if (demoDurum.status === 'active') {
            // Demo devam ediyor
            _lisansGoster('demo', null, demoDurum);
            return;
        }

        // 3. Demo bitmis - kilit ekrani
        _lisansGoster('locked', null, demoDurum);
    });
}

// Lisans durumunu goster
function _lisansGoster(mod, license, demoDurum) {
    var durumKart = document.getElementById('lisansDurumKart');
    var bilgiKart = document.getElementById('lisansBilgiKart');
    var demoBilgi = document.getElementById('lisansDemoBilgi');
    var kilitEkrani = document.getElementById('lisansKilitEkrani');
    var yuklemeAlani = document.getElementById('lisansYuklemeAlani');

    if (!durumKart) return;

    if (mod === 'licensed') {
        durumKart.innerHTML = '<div class="dashboard-card" style="border-left:4px solid #10b981;">' +
            '<div class="card-title">Lisans Durumu</div>' +
            '<div style="display:flex;align-items:center;gap:12px;margin-top:8px;">' +
            '<span style="font-size:1.5rem;">&#x2705;</span>' +
            '<div><div style="font-size:1.1rem;font-weight:700;color:#10b981;">Lisansli Surum</div>' +
            '<div style="font-size:0.85rem;color:#6b7280;">' + ES_LISANS.PRODUCT_NAME + '</div></div></div>';

        // Lisans bilgilerini goster
        _lisansBilgiGoster(license);
        bilgiKart.style.display = 'block';
        demoBilgi.style.display = 'none';
        kilitEkrani.style.display = 'none';
        yuklemeAlani.style.display = 'block'; // Her zaman yukleme acik

        // Dashboard badge
        _dashboardLisansBadge(true);

    } else if (mod === 'demo') {
        var endDate = ES_DEMO.getEndDate();
        var endDateStr = endDate ? endDate.toLocaleDateString('tr-TR') : '-';
        var startDateStr = demoDurum.startDate ? demoDurum.startDate.toLocaleDateString('tr-TR') : '-';

        durumKart.innerHTML = '<div class="dashboard-card" style="border-left:4px solid #f59e0b;">' +
            '<div class="card-title">Lisans Durumu</div>' +
            '<div style="display:flex;align-items:center;gap:12px;margin-top:8px;">' +
            '<span style="font-size:1.5rem;">&#x23f3;</span>' +
            '<div><div style="font-size:1.1rem;font-weight:700;color:#f59e0b;">Deneme Surumu</div>' +
            '<div style="font-size:0.85rem;color:#6b7280;">' + demoDurum.daysLeft + ' gun kaldi</div></div></div>';

        bilgiKart.style.display = 'none';
        kilitEkrani.style.display = 'none';
        yuklemeAlani.style.display = 'block';
        demoBilgi.style.display = 'block';

        // Demo bilgileri
        var demoGrid = document.getElementById('lisansDemoGrid');
        if (demoGrid) {
            demoGrid.innerHTML =
                _lisansSatir('Demo Baslangic', startDateStr) +
                _lisansSatir('Demo Bitis', endDateStr) +
                _lisansSatir('Kalan Gun', '<strong style="color:' + (demoDurum.daysLeft <= 3 ? '#ef4444' : '#f59e0b') + ';">' + demoDurum.daysLeft + ' gun</strong>') +
                _lisansSatir('Urun', ES_LISANS.PRODUCT_NAME) +
                _lisansSatir('Product Code', ES_LISANS.PRODUCT_CODE) +
                _lisansSatir('Makine Kodu', ES_MACHINE.getDisplayId());
        }

        _dashboardLisansBadge(false);

    } else if (mod === 'locked') {
        durumKart.innerHTML = '<div class="dashboard-card" style="border-left:4px solid #ef4444;">' +
            '<div class="card-title">Lisans Durumu</div>' +
            '<div style="display:flex;align-items:center;gap:12px;margin-top:8px;">' +
            '<span style="font-size:1.5rem;">&#x274c;</span>' +
            '<div><div style="font-size:1.1rem;font-weight:700;color:#ef4444;">Sure Dolmus</div>' +
            '<div style="font-size:0.85rem;color:#6b7280;">Gecerli lisans bulunamadi</div></div></div>';

        bilgiKart.style.display = 'none';
        demoBilgi.style.display = 'none';
        kilitEkrani.style.display = 'block';
        yuklemeAlani.style.display = 'block';

        _dashboardLisansBadge(false);
        _modulleriKitle(true);
    }

    // Makine kodu her zaman goster
    _makineKoduGoster();
}

function _lisansBilgiGoster(license) {
    var grid = document.getElementById('lisansBilgiGrid');
    if (!grid || !license) return;

    var supportDate = license.support_expire_date || '-';
    var licenseExpire = license.license_expire_date || 'Suresiz (Perpetual)';
    var policyText = license.license_policy === 'PERPETUAL' ? 'Suresiz (Perpetual)' : 'Abonelik (Subscription)';

    grid.innerHTML =
        _lisansSatir('Lisans Durumu', '<span style="color:#10b981;font-weight:600;">Aktif</span>') +
        _lisansSatir('Firma / Musteri Adi', license.customer_name || '-') +
        _lisansSatir('Musteri Numarasi', license.customer_no || '-') +
        _lisansSatir('Lisans Baslangic', license.issued_at || '-') +
        _lisansSatir('Lisans Bitis', licenseExpire) +
        _lisansSatir('Lisans ID', license.license_id || '-') +
        _lisansSatir('Lisans Politikasi', policyText) +
        _lisansSatir('Lisans Turu', license.license_type || '-') +
        _lisansSatir('Makine Kodu', license.machine_id || ES_MACHINE.getDisplayId()) +
        _lisansSatir('Maks. Transfer', (license.max_transfer_count || '0')) +
        _lisansSatir('Kullanilan Transfer', (license.transfer_count || '0')) +
        _lisansSatir('Notlar', license.notes || '-') +
        _lisansSatir('Urun Adi', license.product_name || '-') +
        _lisansSatir('Product Code', license.product_code || '-') +
        _lisansSatir('Destek Bitis', supportDate);
}

function _lisansSatir(etiket, deger) {
    return '<div class="detay-satir"><span class="detay-etiket">' + etiket + '</span><span class="detay-deger">' + deger + '</span></div>';
}

function _makineKoduGoster() {
    ES_MACHINE.getMachineId().then(function(id) {
        var alan = document.getElementById('lisansMakineKodu');
        if (alan) {
            alan.textContent = id;
            return;
        }

        // Durum kartina ekle
        var kart = document.getElementById('lisansDurumKart');
        if (!kart) return;
        var existing = kart.querySelector('.makine-kodu-alani');
        if (existing) {
            var deger = existing.querySelector('.makine-kodu-deger');
            if (deger) deger.textContent = id;
            return;
        }
        var div = document.createElement('div');
        div.className = 'makine-kodu-alani';
        div.style.cssText = 'margin-top:12px;padding-top:12px;border-top:1px solid #f3f4f6;';
        div.innerHTML = '<div style="font-size:0.78rem;color:#9ca3af;margin-bottom:4px;">Makine Kodu</div>' +
            '<div class="makine-kodu-deger" style="font-size:0.95rem;font-weight:600;color:#1a1a2e;font-family:monospace;letter-spacing:1px;">' +
            id + '</div>' +
            '<div style="font-size:0.72rem;color:#9ca3af;margin-top:4px;">Bu kodu ES Merkez Lisanslama\'ya iletin.</div>';
        kart.querySelector('.dashboard-card').appendChild(div);
    });
}

function _dashboardLisansBadge(isLicensed) {
    var badge = document.getElementById('sidebarLisansBadge');
    if (!badge) {
        var sidebarItem = document.querySelector('.sidebar-menu li a[data-page="lisans"]');
        if (sidebarItem) {
            var span = document.createElement('span');
            span.id = 'sidebarLisansBadge';
            span.className = 'sidebar-badge';
            span.style.cssText = isLicensed ? 'background:#10b981;' : 'background:#f59e0b;';
            span.textContent = isLicensed ? 'AKTIF' : 'DEMO';
            sidebarItem.appendChild(span);
        }
    } else {
        badge.textContent = isLicensed ? 'AKTIF' : 'DEMO';
        badge.style.background = isLicensed ? '#10b981' : '#f59e0b';
    }
}

// ---- LISANS DOSYASI YUKLEME ----
document.addEventListener('DOMContentLoaded', function() {
    var lisansArea = document.getElementById('lisansFileUploadArea');
    var lisansInput = document.getElementById('lisansFileInput');
    var lisansContainer = document.getElementById('lisansUploadedFiles');

    if (lisansArea && lisansInput && lisansContainer) {
        lisansArea.addEventListener('click', function() {
            lisansInput.click();
        });

        lisansInput.addEventListener('change', function() {
            lisansContainer.innerHTML = '';
            var files = Array.from(this.files);
            files.forEach(function(file, index) {
                var item = document.createElement('div');
                item.className = 'uploaded-file-item';
                item.innerHTML = '<span class="file-name">' + (index + 1) + '. ' + file.name + '</span>' +
                    '<span class="file-remove" onclick="this.parentElement.remove()">Kaldir</span>';
                lisansContainer.appendChild(item);
            });
        });
    }

    // Baslangic kontrolu
    esLisansBaslangicKontrolu();
});

// Lisans dosyasini yukle ve dogrula
function lisansDosyaYukle() {
    var input = document.getElementById('lisansFileInput');
    var sonuc = document.getElementById('lisansSonuc');
    var container = document.getElementById('lisansUploadedFiles');

    if (!input || !input.files || input.files.length === 0) {
        if (sonuc) {
            sonuc.innerHTML = '<div style="padding:12px;background:#fee2e2;border-radius:6px;color:#ef4444;">Lutfen bir lisans dosyasi secin.</div>';
        }
        return;
    }

    var file = input.files[0];
    var reader = new FileReader();

    reader.onload = function(e) {
        var content = e.target.result;
        ES_LICENSE.loadAndVerify(content).then(function(result) {
            if (sonuc) {
                if (result.success) {
                    sonuc.innerHTML = '<div style="padding:12px;background:#d1fae5;border-radius:6px;color:#059669;font-weight:600;">' +
                        '&#x2705; ' + result.message + '</div>';
                    // Sayfayi yenile
                    esLisansBaslangicKontrolu();
                } else {
                    var errorClass = result.step >= 6 ? '#ef4444' : '#f59e0b';
                    sonuc.innerHTML = '<div style="padding:12px;background:#fee2e2;border-radius:6px;color:' + errorClass + ';">' +
                        '&#x274c; ' + result.message + '</div>';
                }
            }
        });
    };

    reader.onerror = function() {
        if (sonuc) {
            sonuc.innerHTML = '<div style="padding:12px;background:#fee2e2;border-radius:6px;color:#ef4444;">Dosya okunamadi.</div>';
        }
    };

    reader.readAsText(file);
}

// ---- MODUL KILITLEME ----
// Demo bittiginde calisir
var _modullerKilitli = false;

function _modulleriKitle(kilitli) {
    _modullerKilitli = kilitli;
    if (!kilitli) return;

    // Kilidi acik kalacak sayfalar: lisans, yardim, veri-yedek
    var kilitliSayfalar = ['dashboard', 'paylasimlar', 'medya', 'yayin-gecmisi', 'raporlar', 'bildirim', 'ayarlar'];
    var acikSayfalar = ['lisans', 'yardim', 'veri-yedek'];

    kilitliSayfalar.forEach(function(page) {
        var link = document.querySelector('.sidebar-menu a[data-page="' + page + '"]');
        if (link) {
            link.style.opacity = '0.5';
            link.style.cursor = 'not-allowed';
            link.onclick = function(e) {
                e.preventDefault();
                // Lisans sayfasina yonlendir
                if (typeof navigateTo === 'function') {
                    navigateTo('lisans');
                }
                // Uyari goster
                _kilitUyarisiGoster();
            };
        }
    });

    acikSayfalar.forEach(function(page) {
        var link = document.querySelector('.sidebar-menu a[data-page="' + page + '"]');
        if (link) {
            link.style.opacity = '1';
            link.style.cursor = 'pointer';
        }
    });
}

function _kilitUyarisiGoster() {
    var existing = document.getElementById('kilitUyariModal');
    if (existing) existing.remove();

    var overlay = document.createElement('div');
    overlay.id = 'kilitUyariModal';
    overlay.style.cssText = 'position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(15,23,42,0.55);display:flex;align-items:center;justify-content:center;z-index:9999;';

    var modal = document.createElement('div');
    modal.style.cssText = 'background:#ffffff;border-radius:10px;max-width:420px;width:90%;box-shadow:0 20px 40px rgba(0,0,0,0.25);padding:24px;text-align:center;';

    modal.innerHTML =
        '<div style="font-size:2rem;margin-bottom:8px;">&#x1f512;</div>' +
        '<div style="font-size:1.05rem;font-weight:700;color:#1a1a2e;margin-bottom:8px;">Erisim Kisitlandi</div>' +
        '<div style="font-size:0.88rem;color:#6b7280;line-height:1.5;margin-bottom:18px;">Deneme suresi doldu veya gecerli bir lisans bulunamadi. Calismaya devam etmek icin gecerli bir lisans dosyasi yukleyin.</div>' +
        '<button class="btn btn-primary" style="padding:10px 18px;" onclick="kilitUyarisiKapat()">Lisans Sayfasina Git</button>';

    overlay.appendChild(modal);
    document.body.appendChild(overlay);
}

// Kilitleme uyarisini kapatir ve lisans sayfasina yonlendirir.
function kilitUyarisiKapat() {
    var ov = document.getElementById('kilitUyariModal');
    if (ov) ov.remove();
    if (typeof navigateTo === 'function') {
        navigateTo('lisans');
    }
}
