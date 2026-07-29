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

function setupFileUpload(prefix) {
    var area = document.getElementById(prefix + 'UploadArea');
    var input = document.getElementById(prefix + 'FileInput');
    var container = document.getElementById(prefix + 'UploadedFiles');
    if (!area || !input || !container) return;

    area.addEventListener('click', function() {
        input.click();
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
function simulateSave(type) {
    var names = {
        'standart': 'Standart Paylasim',
        'kampanya': 'Kampanya Paylasimi',
        'detayli': 'Detayli Paylasim'
    };
    var siraNo = String(Math.floor(Math.random() * 899) + 100);
    gecmisSMKayitEkle({
        tarihSaat: new Date().toLocaleString('tr-TR'),
        tur: names[type],
        siraNumarasi: type === 'kampanya' || type === 'detayli' ? '001' : siraNo,
        baslik: names[type] + ' kaydi',
        gorselAdi: 'Gorsel dosyasi (simule)',
        platform: 'Instagram, Facebook, LinkedIn, X, TikTok, Pinterest, YouTube',
        sablon: 'Standart',
        platformCikti: 'Platforma ozel duzenleme (simule)',
        durum: 'bekliyor',
        icerik: names[type] + ' icerigi kaydedildi ve otomatik yayin sirasina eklendi.',
        baglanti: '',
        hataNedeni: ''
    });
    if (type === 'standart') {
        otomatikStandartEkle(names[type] + ' kaydi', siraNo, 'Gorsel (simule)');
        alert(names[type] + ' kaydedildi. Sira numarasi: ' + siraNo + '. Otomatik yayin sirasina eklendi ve mevcut sira numarasi ile dongude kalacak.');
    } else if (type === 'kampanya') {
        var bugun = new Date();
        var bitis = new Date(bugun);
        bitis.setDate(bitis.getDate() + 30);
        otomatikKampanyaEkle(names[type] + ' kaydi', bugun.toISOString().split('T')[0], bitis.toISOString().split('T')[0], 'Gorsel (simule)');
        alert(names[type] + ' kaydedildi. Kampanya baslangic ve bitis tarihleri arasinda otomatik yayinlanacak. Standart Paylasim dongusune dahil edilmez.');
    } else {
        alert(names[type] + ' kaydedildi. Sira numarasi alindi. Standart Paylasim dongusune dahil edilmez.');
    }
}

function simulateNow(type) {
    var names = {
        'standart': 'Standart Paylasim',
        'kampanya': 'Kampanya Paylasimi',
        'detayli': 'Detayli Paylasim',
        'duyuru': 'Duyuru ve Ilanlar'
    };
    var siraNo = (type === 'duyuru') ? '' : String(Math.floor(Math.random() * 899) + 100);
    gecmisSMKayitEkle({
        tarihSaat: new Date().toLocaleString('tr-TR'),
        tur: names[type] + ' (Manuel)',
        siraNumarasi: siraNo,
        baslik: names[type] + ' manuel yayini',
        gorselAdi: 'Gorsel dosyasi (simule)',
        platform: 'Instagram, Facebook, LinkedIn, X, TikTok, Pinterest, YouTube',
        sablon: 'Standart',
        platformCikti: 'Platforma ozel duzenleme (simule)',
        durum: 'basarili',
        icerik: names[type] + ' icerigi manuel olarak aninda yayinlandi.',
        baglanti: 'Manuel yayin. Gercek baglanti teknik sartnamede tanimlanacaktir.',
        hataNedeni: ''
    });
    if (type === 'standart') {
        alert(names[type] + ' tum bagli sosyal medya hesaplarinda aninda yayinlandi. Standart Paylasim otomatik donguden cikarilmadi, sirasinda kalmaya devam ediyor.');
    } else if (type === 'kampanya') {
        alert(names[type] + ' tum bagli sosyal medya hesaplarinda aninda yayinlandi. Kampanya bitis tarihine kadar otomatik dongude kalmaya devam ediyor.');
    } else {
        alert(names[type] + ' tum bagli sosyal medya hesaplarinda aninda yayinlandi.');
    }
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

    var bolumAdi = document.getElementById('webBolum').options[document.getElementById('webBolum').selectedIndex].text;
    var taslak = {
        id: Date.now(),
        icerik: icerik,
        baslik: baslik,
        bolum: bolum,
        bolumAdi: bolum !== '' ? bolumAdi : 'Belirtilmemis',
        tarih: new Date().toLocaleString('tr-TR')
    };
    webTaslaklar.push(taslak);
    webTaslakListele();
    alert('Taslak kaydedildi.');

    document.getElementById('webIcerik').value = '';
    document.getElementById('webBaslik').value = '';
    document.getElementById('webBolum').value = '';
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
    webYayinla(t.icerik, t.baslik, t.bolum);
}

// ===== FAZ 3 - WEB SIMDI YAYINLA =====
function webSimdiYayinla() {
    var icerik = document.getElementById('webIcerik').value.trim();
    var baslik = document.getElementById('webBaslik').value.trim();
    var bolum = document.getElementById('webBolum').value;

    if (icerik === '' && baslik === '') {
        alert('Yayinlanacak icerik veya baslik girin.');
        return;
    }
    if (!bolum || bolum === '') {
        alert('Web bolumu secimi zorunludur. Lutfen bir web bolumu secin.');
        return;
    }

    webYayinla(icerik, baslik, bolum);
}

function webYayinla(icerik, baslik, bolum) {
    var bolumSelect = document.getElementById('webBolum');
    var bolumAdi = bolumSelect.options[bolumSelect.selectedIndex].text;
    var msg = 'Web sitesi yayinlama entegrasyonu henuz yapilandirilmemistir. Icerik yayinlanamadi.\n\nHedef: ' + bolumAdi + '\nBaslik: ' + (baslik || '(baslik yok)');
    alert(msg);
}

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
        html += '<div style="width:120px;padding:12px;background:#f9fafb;border:1px solid #e5e7eb;border-radius:6px;text-align:center;"><div style="font-size:2rem;margin-bottom:6px;">ğŸ–¼ï¸</div><div style="font-size:0.72rem;color:#374151;word-break:break-all;">' + d.ad + '</div><div style="font-size:0.65rem;color:#9ca3af;margin-top:4px;">' + d.tarih + '</div><div style="margin-top:6px;"><span class="file-remove" onclick="medyaDosyaSil(' + i + ')">Sil</span></div></div>';
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
}

function gecmisWebKayitEkle(kayit) {
    yayinGecmisi.webSitesi.unshift(kayit);
    gecmisWebListele();
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
    standartKayitlar: [],   // { siraNo, baslik, tur, tarih, gorselAdi }
    kampanyaKayitlar: [],   // { baslik, baslangic, bitis, tur, tarih, gorselAdi }
    sonCalisma: null
};

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
        yHtml += '<div style="font-size:0.78rem;color:#9ca3af;margin-top:8px;">Otomatik yayin simule edildiginde gecerli entegrasyon bulunmadigi icin basarisiz kaydedilir ve kotadan dusulmez.</div>';
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

function otomatikSimuleEt() {
    if (!otomatikSistem.aktif) {
        alert('Otomatik yayin sistemi duraklatilmistir. Once sistemi devam ettirin.');
        return;
    }

    if (otomatikSistem.bugunKullanilan >= otomatikSistem.gunlukKota) {
        alert('Gunluk otomatik paylasim kotasi (5) dolmustur. Kota doldugu icin yeni otomatik yayin yapilamaz.');
        return;
    }

    // Bagli hesap kontrolu (simule - bagli hesap yok)
    // Entegrasyon olmadigi icin basarisiz kaydedilir, kotadan dusulmez
    var simdi = new Date().toLocaleString('tr-TR');
    var yapilanKayit = null;
    var kayitTuru = '';
    var siraNo = '';

    // Once standart paylasimlara bak
    if (otomatikSistem.standartKayitlar.length > 0) {
        otomatikSistem.standartKayitlar.sort(function(a, b) {
            return (parseInt(a.siraNo, 10) || 0) - (parseInt(b.siraNo, 10) || 0);
        });
        var kayit = otomatikSistem.standartKayitlar.shift();
        kayitTuru = 'Standart Paylasim (Otomatik)';
        siraNo = kayit.siraNo;
        yapilanKayit = kayit;
    } else if (otomatikAktifKampanyaVar()) {
        // Kampanya bul
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

    // Yayin Gecmisi FAZ 5'e kaydet - entegrasyon yok, basarisiz
    if (yapilanKayit) {
        gecmisSMKayitEkle({
            tarihSaat: simdi,
            tur: kayitTuru,
            siraNumarasi: siraNo,
            baslik: yapilanKayit.baslik || '(baslik yok)',
            gorselAdi: yapilanKayit.gorselAdi || 'Gorsel (simule)',
            platform: 'Instagram, Facebook, LinkedIn, X, TikTok, Pinterest, YouTube',
            sablon: 'Standart',
            platformCikti: 'Platforma ozel duzenleme (entegrasyon yok)',
            durum: 'basarisiz',
            icerik: 'Otomatik yayin denemesi. Paylasim turu: ' + kayitTuru + '.',
            baglanti: '',
            hataNedeni: 'Sosyal medya entegrasyonu bulunmadigi icin yayinlanamadi. Gercek entegrasyon kuruldugunda otomatik yayinlar calisacaktir.',
            otomatik: true
        });

        // Kotadan dusulmez - entegrasyon yok
        alert('Otomatik yayin simule edildi ancak sosyal medya entegrasyonu bulunmadigi icin yayinlanamadi. Basarisiz kayit Yayin Gecmisi\'ne eklendi. Gunluk kotadan dusulmedi. (Kota: ' + otomatikSistem.bugunKullanilan + '/' + otomatikSistem.gunlukKota + ')');
    } else {
        alert('Yayinlanacak uygun kayit bulunamadi. (Siradaki standart paylasim yok, aktif kampanya yok)');
    }

    otomatikDurumuGuncelle();
}

// Standart paylasim kaydedildiginde otomatik siraya ekle
function otomatikStandartEkle(baslik, siraNo, gorselAdi) {
    otomatikSistem.standartKayitlar.push({
        siraNo: siraNo,
        baslik: baslik,
        tur: 'Standart Paylasim',
        tarih: new Date().toISOString(),
        gorselAdi: gorselAdi || 'Gorsel (simule)'
    });
    otomatikDurumuGuncelle();
}

// Kampanya kaydedildiginde
function otomatikKampanyaEkle(baslik, baslangic, bitis, gorselAdi) {
    otomatikSistem.kampanyaKayitlar.push({
        baslik: baslik,
        baslangic: baslangic,
        bitis: bitis,
        tur: 'Kampanya',
        tarih: new Date().toISOString(),
        gorselAdi: gorselAdi || 'Gorsel (simule)'
    });
    otomatikDurumuGuncelle();
}

// Sayfa yuklendiginde
document.addEventListener('DOMContentLoaded', function() {
    otomatikGunlukKontrol();
    otomatikDurumuGuncelle();
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
    // Bildirim Ozeti kartini bul - dashboard-grid icinde "Bildirim Ã–zeti" baslikli karti bul
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
// simulateNow - Sosyal medya baglantisi yok uyarisi
(function() {
    var originalSimulateNow = simulateNow;
    simulateNow = function(type) {
        originalSimulateNow(type);
        // Bagli hesap bildirimi
        bildirimEkle('sosyal-medya-baglanti', 'uyari',
            'Sosyal medya baglantisi bulunmuyor',
            'Hicbir sosyal medya hesabi bagli degil. Yayinlar simule ediliyor, gercek platformlara gonderilmiyor.'
        );
    };
})();

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

// Otomatik yayin sonucu bildirimleri
(function() {
    var originalSimule = otomatikSimuleEt;

    otomatikSimuleEt = function() {
        var oncekiKota = otomatikSistem.bugunKullanilan;
        var yayinlanabilirKayitVardi =
            otomatikSistem.aktif &&
            oncekiKota < otomatikSistem.gunlukKota &&
            (otomatikSistem.standartKayitlar.length > 0 || otomatikAktifKampanyaVar());

        originalSimule();

        if (yayinlanabilirKayitVardi) {
            bildirimEkle('yayin-hatasi', 'hata',
                'Otomatik yayin basarisiz',
                'Entegrasyon bulunmadigi icin otomatik yayin yapilamadi. Tarih: ' +
                new Date().toLocaleString('tr-TR')
            );
        }

        if (
            otomatikSistem.bugunKullanilan >= otomatikSistem.gunlukKota &&
            oncekiKota < otomatikSistem.gunlukKota
        ) {
            bildirimEkle('sistem-uyari', 'uyari',
                'Gunluk otomatik kota doldu',
                'Bugunku otomatik paylasim kotasi (5) dolmustur. Yeni otomatik yayin yarin yapilacak.'
            );
        }
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
    { id: 'pinterest', ad: 'Pinterest', bagli: false, hesapAdi: '', sonKontrol: '' },
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
function ayarlarPlatformListele() {
    var liste = document.getElementById('ayarlarPlatformListesi');
    if (!liste) return;

    var html = '';
    ayarlarPlatformlar.forEach(function(p) {
        var durumClass = p.bagli ? 'green' : 'gray';
        var durumText = p.bagli ? 'Bagli' : 'Bagli Degil';
        var hesapBilgi = p.bagli ? ('Hesap: ' + (p.hesapAdi || 'Belirtilmemis')) : '-';
        var sonKontrolText = p.sonKontrol ? ('Son kontrol: ' + p.sonKontrol) : '';

        html += '<div class="ayarlar-platform-kart">';
        html += '    <div class="platform-ust">';
        html += '        <span class="platform-ad">' + p.ad + '</span>';
        html += '        <span class="platform-durum"><span class="status-dot ' + durumClass + '"></span>' + durumText + '</span>';
        html += '    </div>';
        html += '    <div class="platform-alt">';
        html += '        <div class="platform-hesap-input">';
        html += '            <input type="text" class="form-input" id="ayarlarHesapAdi_' + p.id + '" placeholder="Hesap adi" value="' + (p.hesapAdi || '') + '" ' + (p.bagli ? 'disabled' : '') + '>';
        html += '        </div>';
        if (sonKontrolText) {
            html += '        <span class="platform-son-kontrol">' + sonKontrolText + '</span>';
        }
        html += '    </div>';
        html += '    <div class="platform-actions">';
        if (!p.bagli) {
            html += '        <button class="btn btn-primary btn-small" onclick="ayarlarPlatformBaglan(\'' + p.id + '\')">Baglan</button>';
            html += '        <button class="btn btn-warning btn-small" disabled style="opacity:0.4;cursor:not-allowed;">Baglantiyi Kes</button>';
        } else {
            html += '        <button class="btn btn-primary btn-small" disabled style="opacity:0.4;cursor:not-allowed;">Baglan</button>';
            html += '        <button class="btn btn-warning btn-small" onclick="ayarlarPlatformKes(\'' + p.id + '\')">Baglantiyi Kes</button>';
        }
        html += '    </div>';
        html += '</div>';
    });

    liste.innerHTML = html;
}

function ayarlarPlatformBaglan(id) {
    var p = null;
    for (var i = 0; i < ayarlarPlatformlar.length; i++) {
        if (ayarlarPlatformlar[i].id === id) {
            p = ayarlarPlatformlar[i];
            break;
        }
    }
    if (!p) return;

    // Hesap adi al
    var hesapAdi = prompt(p.ad + ' hesap adini girin:', '');
    if (hesapAdi === null) return; // iptal
    hesapAdi = hesapAdi.trim();
    if (hesapAdi === '') {
        alert('Hesap adi bos birakilamaz.');
        return;
    }

    alert('Bu platformun resmi baglanti entegrasyonu henuz yapilandirilmamistir.');
    bildirimEkle('sosyal-medya-baglanti', 'uyari',
        p.ad + ' baglantisi kurulamadi',
        p.ad + ' hesabi icin resmi baglanti entegrasyonu henuz yapilandirilmamistir. Girilen hesap: ' + hesapAdi
    );
}

function ayarlarPlatformKes(id) {
    var p = null;
    for (var i = 0; i < ayarlarPlatformlar.length; i++) {
        if (ayarlarPlatformlar[i].id === id) {
            p = ayarlarPlatformlar[i];
            break;
        }
    }
    if (p && p.bagli) {
        p.bagli = false;
        p.hesapAdi = '';
        p.sonKontrol = '';
        ayarlarPlatformListele();
        dashboardBaglantiGuncelle();
    }
}

// ===== WEB SITESI BAGLANTISI =====
function ayarlarWebTestEt() {
    var webAdres = document.getElementById('ayarlarWebAdres').value.trim();
    var apiAdres = document.getElementById('ayarlarApiAdres').value.trim();
    var authYontem = document.getElementById('ayarlarAuthYontem').value;

    if (!webAdres || !apiAdres) {
        alert('Web sitesi adresi ve API adresi zorunludur.');
        bildirimEkle('sistem-uyari', 'uyari',
            'Web baglantisi testi basarisiz - Zorunlu alan eksik',
            'Web sitesi adresi ve API adresi girilmeden baglanti testi yapilamaz.'
        );
        return;
    }

    if (!authYontem || authYontem === '') {
        alert('Kimlik dogrulama yontemi secimi zorunludur.');
        bildirimEkle('sistem-uyari', 'uyari',
            'Web baglantisi testi basarisiz - Yontem secilmedi',
            'Kimlik dogrulama yontemi secilmeden baglanti testi yapilamaz.'
        );
        return;
    }

    alert('Web sitesi baglanti bilgileri teknik entegrasyon tamamlanmadan dogrulanamaz.');
    bildirimEkle('web-baglanti', 'uyari',
        'Web baglantisi test edilemiyor',
        'Web sitesi baglanti bilgileri teknik entegrasyon tamamlanmadan dogrulanamaz.'
    );
}

function ayarlarWebKaydet() {
    var webAdres = document.getElementById('ayarlarWebAdres').value.trim();
    var apiAdres = document.getElementById('ayarlarApiAdres').value.trim();
    var authYontem = document.getElementById('ayarlarAuthYontem').value;

    if (!webAdres || !apiAdres || !authYontem || authYontem === '') {
        alert('Web sitesi adresi, API adresi ve dogrulama yontemi zorunludur.');
        bildirimEkle('sistem-uyari', 'uyari',
            'Web baglantisi kaydedilemedi - Zorunlu alan eksik',
            'Web sitesi adresi, API adresi ve dogrulama yontemi girilmeden kaydedilemez.'
        );
        return;
    }

    ayarlarWebBaglanti.webAdres = webAdres;
    ayarlarWebBaglanti.apiAdres = apiAdres;
    ayarlarWebBaglanti.authYontem = authYontem;
    ayarlarWebBaglanti.bagli = false;
    ayarlarWebBaglanti.sonKontrol = new Date().toLocaleString('tr-TR');

    // localStorage'a kaydet (API anahtari dahil edilmez)
    var kayit = {
        bagli: ayarlarWebBaglanti.bagli,
        webAdres: webAdres,
        apiAdres: apiAdres,
        authYontem: authYontem,
        sonKontrol: ayarlarWebBaglanti.sonKontrol
    };
    localStorage.setItem(AYARLAR_WEB_KEY, JSON.stringify(kayit));

    // API anahtari localStorage'a kaydedilmez, console'a yazilmaz
    document.getElementById('ayarlarApiAnahtar').value = '';
    ayarlarWebDurumGoster();
    dashboardBaglantiGuncelle();

    alert('Web sitesi baglanti bilgileri kaydedildi. API anahtari saklanmamistir.');

    bildirimEkle('genel', 'basarili',
        'Web baglantisi kaydedildi',
        'Web sitesi baglanti bilgileri basariyla kaydedildi. API anahtari guvenlik nedeniyle saklanmamistir.'
    );
}

function ayarlarWebKaldir() {
    ayarlarWebBaglanti = {
        bagli: false,
        webAdres: '',
        apiAdres: '',
        authYontem: 'API Key',
        sonKontrol: ''
    };

    document.getElementById('ayarlarWebAdres').value = '';
    document.getElementById('ayarlarApiAdres').value = '';
    document.getElementById('ayarlarAuthYontem').value = 'API Key';
    document.getElementById('ayarlarApiAnahtar').value = '';

    localStorage.removeItem(AYARLAR_WEB_KEY);

    ayarlarWebDurumGoster();
    dashboardBaglantiGuncelle();

    alert('Web sitesi baglantisi kaldirildi.');
}

function ayarlarWebDurumGoster() {
    var durumGrubu = document.getElementById('ayarlarWebDurumGrubu');
    var durumEl = document.getElementById('ayarlarWebDurum');
    var sonKontrolEl = document.getElementById('ayarlarWebSonKontrol');
    var kaldirBtn = document.getElementById('ayarlarWebKaldirBtn');

    if (!durumGrubu || !durumEl || !sonKontrolEl || !kaldirBtn) return;

    if (ayarlarWebBaglanti.webAdres) {
        durumGrubu.style.display = 'block';
        durumEl.innerHTML = '<span class="status-dot gray"></span> Bagli Degil';
        sonKontrolEl.textContent = 'Son kontrol: ' + (ayarlarWebBaglanti.sonKontrol || '-');
        kaldirBtn.style.display = 'inline-block';
    } else {
        durumGrubu.style.display = 'none';
        kaldirBtn.style.display = 'none';
    }
}

function ayarlarWebGeriYukle() {
    var kayitli = localStorage.getItem(AYARLAR_WEB_KEY);
    if (kayitli) {
        try {
            var data = JSON.parse(kayitli);
            ayarlarWebBaglanti.bagli = data.bagli || false;
            ayarlarWebBaglanti.webAdres = data.webAdres || '';
            ayarlarWebBaglanti.apiAdres = data.apiAdres || '';
            ayarlarWebBaglanti.authYontem = data.authYontem || 'API Key';
            ayarlarWebBaglanti.sonKontrol = data.sonKontrol || '';

            if (ayarlarWebBaglanti.webAdres) {
                document.getElementById('ayarlarWebAdres').value = ayarlarWebBaglanti.webAdres;
                document.getElementById('ayarlarApiAdres').value = ayarlarWebBaglanti.apiAdres;
                document.getElementById('ayarlarAuthYontem').value = ayarlarWebBaglanti.authYontem;
            }

            ayarlarWebDurumGoster();
        } catch(e) {}
    }
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
    dashboardBaglantiGuncelle();
});

// Dashboard baglanti durumunu Ayarlar verisiyle senkronize et
function dashboardBaglantiGuncelle() {
    // Sosyal medya hesaplari
    ayarlarPlatformlar.forEach(function(p) {
        // Statik HTML'deki sirali yapiyi kullan
        var items = document.querySelectorAll('#dash-sosyal-medya .dashboard-card .status-item');
        var platformSirasi = { 'instagram': 0, 'facebook': 1, 'linkedin': 2, 'x': 3, 'tiktok': 4, 'pinterest': 5, 'youtube': 6 };
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
    PRODUCT_CODE: 'ESOPS',
    PRODUCT_NAME: 'ES Otomatik Paylasim Sistemi',
    DEMO_GUN: 15,
    LICENCE_FILE_NAME: 'license.lic',

    // Public Key - ES Merkez Lisanslama tarafindan saglanacak
    // NOT: Bu deger su an icin yapilandirilmamistir (null).
    // Gercek Ed25519 Public Key gelmedigi surece imza dogrulamasi yapilamaz.
    // Public Key temin edilene kadar tum lisans yuklemeleri 'imza dogrulanamadi'
    // hatasi ile reddedilir. Bu bilincli bir guvenlik onlemidir.
    ED25519_PUBLIC_KEY_BASE64: null
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
// NOT: Gercek CPU ID ve Disk Serial bilgisine erismek icin arka plan servisi gerekir.
// Web tarayicisinda (client-side JS) dogrudan CPU ID ve Disk Serial alinamaz.
// UYGULAMA TURU: Saf HTML/CSS/JS frontend (backend yok). Bu nedenle:
//
// 1. Machine ID olarak tarayici parmak izi (browser fingerprint) kullanilir.
// 2. Gercek makine kimligi atamasi icin asagidaki yontemlerden biri kullanilmalidir:
//    a) Node.js arka plan servisi (electron, NW.js vb.)
//    b) Windows Registry uzerinden WMI/C++ yardimcisi
//    c) ES Merkez Lisanslama tarafindan manuel makine kodu atamasi
// 3. Mevcut cozum GECICI olup, guvenli degildir.
//    Tarayici parmak izi, ayni bilgisayarda farkli tarayicilarda degisir.
//    localStorage silindiginde sifirlanir.
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

    getMachineId: function() {
        if (this._machineId) return this._machineId;

        var stored = ES_DEPO.get('machine_id', null);
        if (stored) {
            this._machineId = stored;
            return stored;
        }

        var id = this._getFallbackId();
        this._machineId = id;
        ES_DEPO.set('machine_id', id);
        return id;
    },

    getDisplayId: function() {
        return this.getMachineId();
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
// Public Key henuz yapilandirilmadigi icin imza dogrulamasi basarisiz olur.
var ES_LICENSE = {
    _currentLicense: null,

    // lisans.lic dosyasini yukle ve dogrula
    loadAndVerify: function(fileContent) {
        var result = {
            success: false,
            message: '',
            license: null,
            step: 0
        };

        // Adim 1: Dosya var mi?
        if (!fileContent || fileContent.trim() === '') {
            result.message = 'Lisans dosyasi bulunamadi.';
            return result;
        }
        result.step = 1;

        // Adim 2: JSON gecerli mi?
        var license;
        try {
            license = JSON.parse(fileContent);
        } catch(e) {
            result.message = 'Gecersiz lisans dosyasi.';
            return result;
        }
        result.step = 2;

        // Adim 3: Dijital imza gecerli mi? (Ed25519)
        if (!this._verifySignature(license)) {
            result.message = 'Lisans dogrulanamadi.';
            return result;
        }
        result.step = 3;

        // Adim 4: Product Code eslesiyor mu?
        if (!license.product_code || license.product_code !== ES_LISANS.PRODUCT_CODE) {
            result.message = 'Bu lisans farkli bir urun icin uretilmistir.';
            return result;
        }
        result.step = 4;

        // Adim 5: Machine ID eslesiyor mu?
        var currentMachineId = ES_MACHINE.getMachineId();
        if (!license.machine_id || license.machine_id !== currentMachineId) {
            result.message = 'Bu lisans bu bilgisayar icin gecerli degildir.';
            return result;
        }
        result.step = 5;

        // Adim 6: status === ACTIVE mi?
        if (!license.status || license.status !== 'ACTIVE') {
            result.message = 'Lisans durumu gecerli degildir.';
            return result;
        }
        result.step = 6;

        // Adim 7: Lisans politikasi kontrolu
        if (license.license_policy === 'SUBSCRIPTION') {
            if (license.license_expire_date) {
                var expireDate = new Date(license.license_expire_date);
                var now = new Date();
                if (now > expireDate) {
                    result.message = 'Lisans suresi dolmustur.';
                    return result;
                }
            }
        }
        // PERPETUAL ise sure kontrolu yapilmaz
        result.step = 7;

        // Tum kontroller basarili
        this._currentLicense = license;
        result.success = true;
        result.message = 'Lisans basariyla etkinlestirildi.';
        result.license = license;

        // Lisansi kalici olarak kaydet
        this._saveLicense(license);

        return result;
    },

    // Imza dogrulamasi (Ed25519)
    _verifySignature: function(license) {
        if (!license.signature) return false;

        // Public Key yapilandirilmamis ise imza dogrulamasi yapilamaz
        if (!ES_LISANS.ED25519_PUBLIC_KEY_BASE64) {
            console.warn('ES_OPS: Ed25519 Public Key yapilandirilmamistir. Imza dogrulamasi yapilamaz.');
            return false;
        }

        try {
            // Lisans verisinden imza haric JSON olustur
            var dataToVerify = {};
            for (var key in license) {
                if (key !== 'signature') {
                    dataToVerify[key] = license[key];
                }
            }
            var messageBytes = nacl.util.decodeUTF8(JSON.stringify(dataToVerify));
            // Referans license.lic: signature Base64URL formatindadir
            var sigBase64 = license.signature.replace(/-/g, '+').replace(/_/g, '/');
            while (sigBase64.length % 4 !== 0) { sigBase64 += '='; }
            var signatureBytes = nacl.util.decodeBase64(sigBase64);
            var pubK = ES_LISANS.ED25519_PUBLIC_KEY_BASE64 || '';
            var pubBase64 = pubK.replace(/-/g, '+').replace(/_/g, '/');
            while (pubBase64.length % 4 !== 0) { pubBase64 += '='; }
            var publicKeyBytes = nacl.util.decodeBase64(pubBase64);

            return nacl.sign.detached.verify(messageBytes, signatureBytes, publicKeyBytes);
        } catch(e) {
            console.error('Imza dogrulama hatasi:', e);
            return false;
        }
    },

    // Lisansi localStorage'a kaydet
    _saveLicense: function(license) {
        try {
            localStorage.setItem('es_ops_license_data', JSON.stringify(license));
        } catch(e) {}
    },

    // Kayitli lisansi getir
    getStoredLicense: function() {
        if (this._currentLicense) return this._currentLicense;
        try {
            var stored = localStorage.getItem('es_ops_license_data');
            if (stored) {
                this._currentLicense = JSON.parse(stored);
                return this._currentLicense;
            }
        } catch(e) {}
        return null;
    },

    // Kayitli lisansi temizle
    clearStoredLicense: function() {
        this._currentLicense = null;
        try {
            localStorage.removeItem('es_ops_license_data');
        } catch(e) {}
    },

    // Kayitli lisansi dogrula (program baslangicinda)
    verifyStoredLicense: function() {
        var license = this.getStoredLicense();
        if (!license) return { valid: false, reason: 'no_license' };

        // Product Code kontrol
        if (license.product_code !== ES_LISANS.PRODUCT_CODE) {
            this.clearStoredLicense();
            return { valid: false, reason: 'wrong_product' };
        }

        // Machine ID kontrol
        if (license.machine_id !== ES_MACHINE.getMachineId()) {
            this.clearStoredLicense();
            return { valid: false, reason: 'wrong_machine' };
        }

        // status kontrol
        if (license.status !== 'ACTIVE') {
            this.clearStoredLicense();
            return { valid: false, reason: 'not_active' };
        }

        // Imza kontrolu
        if (!this._verifySignature(license)) {
            this.clearStoredLicense();
            return { valid: false, reason: 'invalid_signature' };
        }

        // Sure kontrolu
        if (license.license_policy === 'SUBSCRIPTION' && license.license_expire_date) {
            var expireDate = new Date(license.license_expire_date);
            var now = new Date();
            if (now > expireDate) {
                this.clearStoredLicense();
                return { valid: false, reason: 'expired' };
            }
        }

        return { valid: true, license: license };
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

    // 1. Kayitli lisans var mi ve gecerli mi?
    var lisansDurum = ES_LICENSE.verifyStoredLicense();

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
        _lisansSatir('Makine Kodu', ES_MACHINE.getDisplayId()) +
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
    var alan = document.getElementById('lisansMakineKodu');
    if (!alan) {
        // Durum kartina ekle
        var kart = document.getElementById('lisansDurumKart');
        if (kart) {
            var existing = kart.querySelector('.makine-kodu-alani');
            if (!existing) {
                var div = document.createElement('div');
                div.className = 'makine-kodu-alani';
                div.style.cssText = 'margin-top:12px;padding-top:12px;border-top:1px solid #f3f4f6;';
                div.innerHTML = '<div style="font-size:0.78rem;color:#9ca3af;margin-bottom:4px;">Makine Kodu</div>' +
                    '<div style="font-size:0.95rem;font-weight:600;color:#1a1a2e;font-family:monospace;letter-spacing:1px;">' +
                    ES_MACHINE.getDisplayId() + '</div>' +
                    '<div style="font-size:0.72rem;color:#9ca3af;margin-top:4px;">Bu kodu ES Merkez Lisanslama\'ya iletin.</div>';
                kart.querySelector('.dashboard-card').appendChild(div);
            }
        }
    }
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
        var result = ES_LICENSE.loadAndVerify(content);

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
    overlay.style.cssText = 'position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.5);z-index:2000;display:flex;align-items:center;justify-content:center;';

    var modal = document.createElement('div');
    modal.style.cssText = 'background:#fff;border-radius:12px;padding:32px;max-width:400px;width:90%;text-align:center;box-shadow:0 8px 30px rgba(0,0,0,0.2);';

    modal.innerHTML =
        '<div style="font-size:3rem;margin-bottom:16px;">&#x1f512;</div>' +
        '<h3 style="color:#ef4444;margin-bottom:12px;">Deneme Sureniz Sona Ermistir</h3>' +
        '<p style="color:#6b7280;margin-bottom:20px;font-size:0.9rem;">' +
        '15 gunluk deneme sureniz sona ermistir. Bu modulu kullanmaya devam etmek icin gecerli bir lisans dosyasi yukleyin.</p>' +
        '<div style="display:flex;gap:10px;justify-content:center;">' +
        '<button onclick="this.closest(\'#kilitUyariModal\').remove()" style="padding:10px 24px;border:none;border-radius:6px;background:#f3f4f6;color:#374151;font-weight:600;cursor:pointer;">Kapat</button>' +
        '<button onclick="navigateTo(\'lisans\');this.closest(\'#kilitUyariModal\').remove()" style="padding:10px 24px;border:none;border-radius:6px;background:#4f8cff;color:#fff;font-weight:600;cursor:pointer;">Lisans Sayfasina Git</button>' +
        '</div>';

    overlay.appendChild(modal);
    document.body.appendChild(overlay);

    overlay.addEventListener('click', function(e) {
        if (e.target === overlay) overlay.remove();
    });
}

// ---- MEVCUT NAVIGATE FONKSIYONUNU EZ - KILIT KONTROLU ----
(function() {
    var originalNavigate = window.navigateTo;
    if (typeof originalNavigate === 'function') {
        window.navigateTo = function(page) {
            if (_modullerKilitli) {
                var acikSayfalar = ['lisans', 'yardim', 'veri-yedek'];
                if (acikSayfalar.indexOf(page) === -1) {
                    _kilitUyarisiGoster();
                    originalNavigate('lisans');
                    return;
                }
            }
            originalNavigate(page);
        };
    }
})();

// ---- LISANS SAYFASI YONLENDIRME (navigateTo icin lisans basligi) ----
(function() {
    var originalTitles = window.navigateTo;
    // Lisans sayfa basligi zaten 'Lisans' olarak tanimli
})();

console.log('ES OPS Lisans Modulu FAZ 9 yuklendi.');
console.log('NOT: Machine ID tarayici tabanli (GECICI) calisir. Referans format: MID-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX');
console.log('NOT: localStorage kullanici tarafindan silinebilir. Demo sayaci sifirlanir.');
console.log('NOT: Ed25519 Public Key temin edilene kadar lisans yuklemeleri basarisiz olur.');
console.log('NOT: Referans license.lic alanlari: customer_name, customer_no, issued_at, license_expire_date, license_id, license_policy, license_type, machine_id, max_transfer_count, notes, product_code, product_name, signature, status, support_expire_date, transfer_count');








// ===== FAZ 10 - SOSYAL MEDYA HESAP BAGLANTISI (ISTEMCI TARAFLI) =====
// Bu modul platformlarin resmi OAuth dokumantasyonuna dayanmaktadir.
// Varsayimsal destek yoktur.
// Yalnizca gercek baglanti.
// Tokenlar projenin mevcut teknolojisiyle OS guvenli kasasina
// yazilamadigi icin hicbir platformda token saklanamaz.
// Bu nedenle hicbir platformda gercek baglanti kurulamaz.
// ===============================================================

// ----------------------------------------------------------------
// PLATFORM ANALIZI (Resmi Dokumantasyona Gore)
// ----------------------------------------------------------------
//
// 1. Facebook (Graph API v19.0)
//    Native PKCE: Evet, PKCE destekler
//    Client secret gereksinimi: Token exchange icin client_secret ZORUNLU DEGILDIR
//      (public client / PKCE ile secret olmadan calisir)
//    Windows callback: loopback IP redirect (http://127.0.0.1:PORT/)
//      veya kayitli URI scheme (fbAPPID://)
//    Android callback: Facebook Login SDK -> intent filter
//    iOS callback: Facebook Login SDK -> URL scheme
//    Gercek paylasim API: Graph API /me/feed, /photos, /videos
//      => CORS: Graph API CORS destekler
//      => Token turu: page access token (sayfa yonetimi icin)
//    Gerekli yayin izinleri: pages_read_engagement, pages_manage_posts
//    SONUC: OAuth baglantisi KURULABILIR (PKCE + secret yok)
//           Paylasim API cagrisi YAPILABILIR (Graph API CORS acik)
//           Token GUIVENLI SAKLANAMAZ (proje teknolojisi yetmez)
//
// 2. LinkedIn (v2 API)
//    Native PKCE: Evet, PKCE destekler
//    Client secret gereksinimi: PKCE ile secret OLMADAN token alinabilir
//    Windows callback: http://localhost:PORT/ veya kayitli URI
//    Android callback: intent deep link
//    iOS callback: URL scheme
//    Gercek paylasim API: /ugcPosts, /shares
//      => CORS: LinkedIn API CORS izin vermiyor (dokumante)
//      => Istemci uygulamadan paylasim API cagrisi basarisiz olur
//    Gerekli yayin izinleri: w_member_social
//    SONUC: OAuth baglantisi KURULABILIR (PKCE var, secret yok)
//           Paylasim YAPILAMAZ (CORS kapali)
//           Token GUIVENLI SAKLANAMAZ
//
// 3. X (Twitter, API v2)
//    Native PKCE: Evet, OAuth 2.0 PKCE destekler
//    Client secret gereksinimi: PKCE ile secret olmadan calisir
//      (public client, code_challenge ile)
//    Windows callback: http://localhost:PORT/ veya kayitli URI
//    Android callback: app link / deep link
//    iOS callback: universal link
//    Gercek paylasim API: POST /2/tweets
//      => CORS: Twitter API v2 CORS izin vermiyor
//      => Istemci uygulamadan tweet gonderilemez
//    Gerekli yayin izinleri: tweet.read, tweet.write
//    SONUC: OAuth baglantisi KURULABILIR (PKCE var)
//           Paylasim YAPILAMAZ (CORS kapali)
//           Token GUIVENLI SAKLANAMAZ
//
// 4. Pinterest (API v5)
//    Native PKCE: Evet, PKCE destekler
//    Client secret gereksinimi: PKCE ile secret olmadan token exchange YAPILAMAZ
//      (Pinterest client_secret ZORUNLU tutar)
//    Windows callback: kayitli redirect URI
//    Android callback: deep link
//    iOS callback: URL scheme
//    Gercek paylasim API: POST /v5/pins
//      => CORS: Pinterest API CORS izin vermiyor
//    Gerekli yayin izinleri: boards:read, pins:read, pins:write
//    SONUC: OAuth baglantisi KURULAMAZ (client_secret zorunlu)
//           Paylasim YAPILAMAZ
//
// 5. YouTube (Google API)
//    Native PKCE: Evet, PKCE destekler
//    Client secret gereksinimi: PKCE ile secret olmadan token alinabilir
//      (desktop uygulamalari icin)
//    Windows callback: http://127.0.0.1:PORT/ (loopback redirect)
//    Android callback: Google Sign-In SDK
//    iOS callback: Google Sign-In SDK / URL scheme
//    Gercek paylasim API: POST /upload/youtube/v3/videos
//      => CORS: Google API CORS izin verir
//      => ANCAK: Video yuklemek icin multipart upload gerekir
//      => Istemci uygulamadan video yukleme mumkun
//    Gerekli yayin izinleri: youtube.upload, youtube.readonly
//    SONUC: OAuth baglantisi KURULABILIR (PKCE var, secret yok)
//           Paylasim YAPILABILIR (CORS acik, upload destegi var)
//           Token GUIVENLI SAKLANAMAZ
//
// 6. Instagram (Basic Display / Graph API)
//    Native PKCE: Yok (Basic Display -> Implicit Flow)
//    Client secret gereksinimi: Graph API icin EVET, zorunlu
//    Business hesap gerektirir
//    SONUC: PASIF - backend gerektirir
//
// 7. TikTok
//    Native PKCE: Yok
//    Client secret gereksinimi: EVET, zorunlu
//    SONUC: PASIF - backend gerektirir
//
// ----------------------------------------------------------------
// KESIN KARAR
// ----------------------------------------------------------------
// Projenin mevcut teknolojisi (duz JavaScript, HTML, CSS, localStorage)
// isletim sistemi guvenli kasa (Windows Credential Manager,
// Android Keystore, iOS Keychain) ile etkilesime gecmez.
// Bu nedenle hicbir platformda token guvenli sekilde saklanamaz.
//
// Ayrica:
// - LinkedIn paylasim API CORS kapali
// - Twitter paylasim API CORS kapali
// - Pinterest client_secret zorunlu
//
// Geriye kalan:
// - Facebook: baglanti kurulabilir, paylasim yapilabilir, token guvenli saklanamaz
// - YouTube:  baglanti kurulabilir, paylasim yapilabilir, token guvenli saklanamaz
//
// Token guvenli saklama katmani hazir olmadigi surece hicbir platform AKTIF
// edilemez. Asagidaki kod sadece yapilandirma bilgilerini icerir.
// Gercek baglanti ve paylasim kodu yoktur.
// ===============================================================

// ---- PLATFORM TANIMLARI (TUMU PASIF) ----
var ES10 = {
    platformlar: {
        facebook: {
            id: 'facebook',
            ad: 'Facebook',
            grup: 'B',
            baglanti: 'mumkun',       // PKCE + secret yok
            paylasim: 'mumkun',        // Graph API CORS acik
            tokenSaklama: 'imkansiz',  // OS guvenli kasaya erisim yok
            durum: 'pasif',
            durumAciklama: 'OAuth baglantisi ve paylasim API\'si calisabilir durumdadir. ' +
                           'Token Windows Credential Manager / Android Keystore / iOS Keychain\'de ' +
                           'saklanmalidir. Projenin mevcut teknolojisi bu kasalara erisemedigi icin ' +
                           'ilk surumde pasiftir.'
        },
        youtube: {
            id: 'youtube',
            ad: 'YouTube',
            grup: 'B',
            baglanti: 'mumkun',
            paylasim: 'mumkun',
            tokenSaklama: 'imkansiz',
            durum: 'pasif',
            durumAciklama: 'OAuth baglantisi ve video yukleme API\'si calisabilir durumdadir. ' +
                           'Token guvenli kasa gerektirir. Mevcut teknoloji ile saklanamaz.'
        },
        linkedin: {
            id: 'linkedin',
            ad: 'LinkedIn',
            grup: 'B',
            baglanti: 'mumkun',
            paylasim: 'imkansiz',      // CORS kapali
            tokenSaklama: 'imkansiz',
            durum: 'pasif',
            durumAciklama: 'OAuth baglantisi kurulabilir ancak paylasim API\'si CORS izni vermez. ' +
                           'Token guvenli saklanamaz. Ilk surumde pasif.'
        },
        twitter: {
            id: 'twitter',
            ad: 'X (Twitter)',
            grup: 'B',
            baglanti: 'mumkun',
            paylasim: 'imkansiz',      // CORS kapali
            tokenSaklama: 'imkansiz',
            durum: 'pasif',
            durumAciklama: 'OAuth baglantisi kurulabilir ancak paylasim API\'si CORS izni vermez. ' +
                           'Token guvenli saklanamaz. Ilk surumde pasif.'
        },
        pinterest: {
            id: 'pinterest',
            ad: 'Pinterest',
            grup: 'B',
            baglanti: 'imkansiz',      // client_secret zorunlu
            paylasim: 'imkansiz',
            tokenSaklama: 'imkansiz',
            durum: 'pasif',
            durumAciklama: 'Pinterest API client_secret zorunlu tutar. ' +
                           'PKCE ile calismaz. Backend gerektirir. Ilk surumde pasif.'
        },
        instagram: {
            id: 'instagram',
            ad: 'Instagram',
            grup: 'B',
            baglanti: 'imkansiz',
            paylasim: 'imkansiz',
            tokenSaklama: 'imkansiz',
            durum: 'pasif',
            durumAciklama: 'Instagram Business hesap ve Facebook Graph API Client Secret gerektirir. ' +
                           'Backend olmadan calismaz. Ilk surumde pasif.'
        },
        tiktok: {
            id: 'tiktok',
            ad: 'TikTok',
            grup: 'B',
            baglanti: 'imkansiz',
            paylasim: 'imkansiz',
            tokenSaklama: 'imkansiz',
            durum: 'pasif',
            durumAciklama: 'TikTok API Client Secret zorunlu tutar. ' +
                           'Backend olmadan calismaz. Ilk surumde pasif.'
        }
    }
};


// ---- PLATFORM BILGILERINI GOSTER ----
// Kullaniciya her platformun durumunu aciklar

function es10PlatformDurum(platformId) {
    var p = ES10.platformlar[platformId];
    if (!p) return 'Bilinmeyen platform: ' + platformId;

    var satirlar = [];
    satirlar.push(p.ad + ' durumu: ' + p.durum.toUpperCase());
    satirlar.push('');
    satirlar.push(p.durumAciklama);

    if (p.baglanti === 'mumkun') {
        satirlar.push('');
        satirlar.push('OAuth baglantisi: MUMKUN (PKCE + public client)');
    }
    if (p.paylasim === 'mumkun') {
        satirlar.push('Gercek paylasim: MUMKUN (API CORS acik)');
    }
    if (p.paylasim === 'imkansiz' && p.baglanti === 'mumkun') {
        satirlar.push('Gercek paylasim: IMKANSIZ (API CORS kapali - istemciden gonderilemez)');
    }
    if (p.tokenSaklama === 'imkansiz') {
        satirlar.push('Token saklama: IMKANSIZ (OS guvenli kasasina erisim icin native kod gerekir)');
    }

    return satirlar.join('\n');
}


// ---- BAGLAN / KES FONKSIYONLARI (TUM PLATFORMLAR PASIF OLDUGU ICIN DEVRE DISI) ----
// FAZ 8'deki orijinal ayarlarPlatformBaglan ve ayarlarPlatformKes kullanilir.
// Bu fonksiyonlar sadece platform durumunu gosterir.

window.ayarlarPlatformBaglan = function(id) {
    var p = null;
    for (var i = 0; i < ayarlarPlatformlar.length; i++) {
        if (ayarlarPlatformlar[i].id === id) {
            p = ayarlarPlatformlar[i];
            break;
        }
    }
    if (!p) return;

    var platform = ES10.platformlar[id];
    if (!platform) {
        alert('Platform tanimli degil: ' + id);
        return;
    }

    if (platform.durum === 'pasif') {
        alert(es10PlatformDurum(id));
        return;
    }

    // Platform aktif olsaydi burada OAuth baslatilirdi
    // Ancak su an hicbir platform aktif degil
    alert(es10PlatformDurum(id));
};

window.ayarlarPlatformKes = function(id) {
    var p = null;
    for (var i = 0; i < ayarlarPlatformlar.length; i++) {
        if (ayarlarPlatformlar[i].id === id) {
            p = ayarlarPlatformlar[i];
            break;
        }
    }
    if (!p) return;
    if (!p.bagli) return;

    // Baglanti varsa kes (FAZ 8'deki orijinal mantik)
    // Su an hicbir platform baglanamadigi icin buraya hic gelinmez
    p.bagli = false;
    p.sonKontrol = '';
    p.baglantiTarihi = '';
    dashboardBaglantiGuncelle();
    ayarlarSosyalListele();
};


// ---- GERCEK PAYLASIM ----
// Ilk surumde hicbir platforma gercek paylasim yok.
// FAZ 10 bu fonksiyona dokunmaz.

// ---- FAZ 10 DURUM BILDIRIMI ----
console.log('=== FAZ 10 - Sosyal Medya Hesap Baglantisi ===');
console.log('Tum platformlar PASIF.');
console.log('Gerekce: Tokenlar OS guvenli kasasinda saklanamaz.');
console.log('Gerekce: Mevcut proje teknolojisi (JS/HTML/CSS) Windows Credential Manager,');
console.log('         Android Keystore veya iOS Keychain ile etkilesime gecmez.');
console.log('Gerekce: LinkedIn, Twitter paylasim API\'leri CORS kapali.');
console.log('Gerekce: Pinterest, Instagram, TikTok client_secret zorunlu.');
console.log('Platformlar:');
console.log('  Facebook  (Aday - baglanti+paylasim mumkun, token saklanamiyor)');
console.log('  YouTube   (Aday - baglanti+paylasim mumkun, token saklanamiyor)');
console.log('  LinkedIn  (OAuth mumkun, paylasim API CORS kapali)');
console.log('  X/Twitter (OAuth mumkun, paylasim API CORS kapali)');
console.log('  Pinterest (client_secret zorunlu)');
console.log('  Instagram (Business hesap + client_secret zorunlu)');
console.log('  TikTok    (client_secret zorunlu)');
console.log('');
console.log('NOT: Baglanti ve paylasim icin native platform ara katmani (Electron/eel/.NET MAUI)');
console.log('     ile OS guvenli kasasina erisim saglanmalidir.');
console.log('NOT: Windows -> Windows Credential Manager (wincred API)');
console.log('NOT: Android -> Android Keystore (KeyStore API)');
console.log('NOT: iOS -> iOS Keychain (Security framework)');
